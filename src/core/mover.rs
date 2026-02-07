use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

const CLOAK_DIR: &str = ".cloak";
const STORAGE_DIR: &str = "storage";

/// Ensure `.cloak/storage/` exists.
pub fn ensure_storage_dir(root: &Path) -> Result<()> {
    let storage = root.join(CLOAK_DIR).join(STORAGE_DIR);
    fs::create_dir_all(&storage)
        .with_context(|| format!("failed to create storage directory: {}", storage.display()))?;
    Ok(())
}

/// Move a path, falling back to copy+delete if rename fails with a cross-device error.
fn move_path(src: &Path, dest: &Path) -> Result<()> {
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device_error(&e) => {
            copy_and_delete(src, dest)?;
            Ok(())
        }
        Err(e) => Err(e)
            .with_context(|| format!("failed to move {} -> {}", src.display(), dest.display())),
    }
}

/// Check if an IO error is a cross-device link error (EXDEV).
fn is_cross_device_error(e: &std::io::Error) -> bool {
    // Rust 1.74+ exposes CrossesDevices; also check raw OS error for EXDEV (errno 18)
    if e.kind() == std::io::ErrorKind::CrossesDevices {
        return true;
    }
    // EXDEV is errno 18 on all Unix-like systems
    #[cfg(unix)]
    if e.raw_os_error() == Some(18) {
        return true;
    }
    false
}

/// Copy src to dest, then delete src. Handles both files and directories.
fn copy_and_delete(src: &Path, dest: &Path) -> Result<()> {
    if src.is_dir() {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.copy_inside = true;
        options.content_only = true;
        fs::create_dir_all(dest).with_context(|| {
            format!("failed to create destination directory: {}", dest.display())
        })?;
        fs_extra::dir::copy(src, dest, &options).with_context(|| {
            format!(
                "cross-device fallback: failed to copy directory {} -> {}",
                src.display(),
                dest.display()
            )
        })?;
        fs::remove_dir_all(src).with_context(|| {
            format!(
                "cross-device fallback: failed to remove source directory: {}",
                src.display()
            )
        })?;
    } else {
        fs::copy(src, dest).with_context(|| {
            format!(
                "cross-device fallback: failed to copy file {} -> {}",
                src.display(),
                dest.display()
            )
        })?;
        fs::remove_file(src).with_context(|| {
            format!(
                "cross-device fallback: failed to remove source file: {}",
                src.display()
            )
        })?;
    }
    Ok(())
}

/// Move a target from project root into `.cloak/storage/`.
pub fn ingest(root: &Path, target: &str) -> Result<()> {
    let src = root.join(target);
    let dest = root.join(CLOAK_DIR).join(STORAGE_DIR).join(target);

    if !src.exists() {
        bail!("target does not exist: {}", src.display());
    }

    if dest.exists() {
        bail!(
            "target already exists in storage: {} (already hidden?)",
            dest.display()
        );
    }

    ensure_storage_dir(root)?;
    move_path(&src, &dest)?;

    Ok(())
}

/// Move a target from `.cloak/storage/` back to project root.
pub fn egest(root: &Path, target: &str) -> Result<()> {
    let src = root.join(CLOAK_DIR).join(STORAGE_DIR).join(target);
    let dest = root.join(target);

    if !src.exists() {
        bail!("target not found in storage: {}", src.display());
    }

    if dest.exists() {
        bail!(
            "target already exists at root: {} (remove the symlink first)",
            dest.display()
        );
    }

    move_path(&src, &dest)?;

    Ok(())
}
