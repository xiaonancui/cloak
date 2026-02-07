use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let pid = std::process::id();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("cloak-it-{prefix}-{pid}-{nanos}-{seq}"));
        fs::create_dir_all(&path).expect("failed to create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn cloak_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_cloak") {
        return PathBuf::from(path);
    }

    let exe_name = if cfg!(windows) { "cloak.exe" } else { "cloak" };
    let current = std::env::current_exe().expect("failed to get current test executable path");
    let guess = current
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join(exe_name))
        .expect("failed to infer binary path");

    if guess.exists() {
        return guess;
    }

    panic!("could not resolve cloak binary path; tried inferred path");
}

fn run_cloak(root: &Path, args: &[&str]) -> Output {
    Command::new(cloak_bin())
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("failed to execute cloak")
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        output_text(output)
    );
}

fn remove_path_entry(path: &Path) {
    let meta = fs::symlink_metadata(path).expect("target path does not exist");

    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        {
            fs::remove_file(path).expect("failed to remove symlink");
            return;
        }

        #[cfg(windows)]
        {
            if meta.is_dir() {
                fs::remove_dir(path).expect("failed to remove directory symlink");
            } else {
                fs::remove_file(path).expect("failed to remove file symlink");
            }
            return;
        }
    }

    if meta.is_dir() {
        fs::remove_dir(path).expect("failed to remove directory");
    } else {
        fs::remove_file(path).expect("failed to remove file");
    }
}

#[test]
fn init_creates_storage_and_gitignore_rules() {
    let root = TempDir::new("init");
    let out = run_cloak(root.path(), &["init"]);
    assert_success(&out);

    assert!(root.path().join(".cloak").join("storage").exists());
    let gitignore =
        fs::read_to_string(root.path().join(".gitignore")).expect("failed to read .gitignore");
    assert!(gitignore.contains("/.cloak/*"));
    assert!(gitignore.contains("!/.cloak/storage/"));
}

#[test]
fn unhide_refuses_when_original_path_is_not_link() {
    let root = TempDir::new("unhide-conflict");
    let cursor = root.path().join(".cursor");
    fs::create_dir_all(&cursor).expect("failed to create .cursor");
    fs::write(cursor.join("settings.json"), "{\"foo\":1}\n").expect("failed to write settings");

    let hide_out = run_cloak(root.path(), &["hide", ".cursor"]);
    assert_success(&hide_out);

    remove_path_entry(&cursor);
    fs::create_dir_all(&cursor).expect("failed to create conflict dir");
    fs::write(cursor.join("local.txt"), "conflict\n").expect("failed to write conflict marker");

    let unhide_out = run_cloak(root.path(), &["unhide", ".cursor"]);
    assert!(
        !unhide_out.status.success(),
        "unhide should fail when root target is not a symlink:\n{}",
        output_text(&unhide_out)
    );

    let combined = output_text(&unhide_out);
    assert!(
        combined.contains("path is not a symlink")
            || combined.contains("target already exists at root"),
        "unexpected error output:\n{}",
        combined
    );

    assert!(
        root.path()
            .join(".cloak")
            .join("storage")
            .join(".cursor")
            .exists(),
        "storage copy should remain after failed unhide"
    );
}

#[cfg(unix)]
#[test]
fn status_reports_orphaned_symlink() {
    let root = TempDir::new("orphan-status");
    let cursor = root.path().join(".cursor");
    fs::create_dir_all(&cursor).expect("failed to create .cursor");
    fs::write(cursor.join("settings.json"), "{\"foo\":1}\n").expect("failed to write settings");

    let hide_out = run_cloak(root.path(), &["hide", ".cursor"]);
    assert_success(&hide_out);

    fs::remove_dir_all(root.path().join(".cloak").join("storage").join(".cursor"))
        .expect("failed to remove storage target");

    let status_out = run_cloak(root.path(), &["status"]);
    assert_success(&status_out);

    let text = String::from_utf8_lossy(&status_out.stdout);
    assert!(
        text.contains("Orphaned symlinks"),
        "status did not report orphaned symlinks:\n{}",
        text
    );
    assert!(
        text.contains(".cursor [broken]"),
        "status did not report broken .cursor link:\n{}",
        text
    );
}

#[cfg(target_os = "linux")]
#[test]
fn hide_and_unhide_work_with_cross_device_storage_symlink() {
    use std::os::unix::fs::{MetadataExt, symlink};

    if !Path::new("/dev/shm").exists() {
        return;
    }

    let root = TempDir::new("cross-device-root");
    let root_dev = fs::metadata(root.path())
        .expect("metadata root failed")
        .dev();
    let shm_dev = fs::metadata("/dev/shm")
        .expect("metadata /dev/shm failed")
        .dev();

    // Skip if /tmp and /dev/shm are unexpectedly on the same device.
    if root_dev == shm_dev {
        return;
    }

    let external = TempDir::new("cross-device-storage");
    let mut external_storage = PathBuf::from("/dev/shm");
    external_storage.push(
        external
            .path()
            .file_name()
            .expect("external temp dir has no file name"),
    );
    fs::create_dir_all(external_storage.join("storage")).expect("failed to create shm storage");

    fs::create_dir_all(root.path().join(".cloak")).expect("failed to create .cloak");
    symlink(
        external_storage.join("storage"),
        root.path().join(".cloak").join("storage"),
    )
    .expect("failed to link .cloak/storage to /dev/shm");

    let cursor = root.path().join(".cursor");
    fs::create_dir_all(&cursor).expect("failed to create .cursor");
    fs::write(cursor.join("settings.json"), "{\"foo\":1}\n").expect("failed to write settings");

    let hide_out = run_cloak(root.path(), &["hide", ".cursor"]);
    assert_success(&hide_out);

    assert!(
        external_storage.join("storage").join(".cursor").exists(),
        "cross-device storage target missing after hide"
    );

    let unhide_out = run_cloak(root.path(), &["unhide", ".cursor"]);
    assert_success(&unhide_out);

    assert!(
        root.path().join(".cursor").is_dir(),
        "root .cursor should be restored after unhide"
    );
    assert!(
        !external_storage.join("storage").join(".cursor").exists(),
        "external storage should be empty after unhide"
    );

    let _ = fs::remove_dir_all(external_storage);
}
