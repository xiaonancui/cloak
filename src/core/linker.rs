use anyhow::{bail, Context, Result};
use std::path::Path;

/// Create a symlink at the original location pointing to `.cloak/storage/<target>`.
pub fn create_ghost_link(root: &Path, target: &str) -> Result<()> {
    let link_path = root.join(target);
    let storage_path = root.join(".cloak").join("storage").join(target);

    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        bail!(
            "cannot create symlink: path already exists at {}",
            link_path.display()
        );
    }

    if !storage_path.exists() {
        bail!(
            "storage target does not exist: {}",
            storage_path.display()
        );
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&storage_path, &link_path).with_context(|| {
            format!(
                "failed to create symlink {} -> {}",
                link_path.display(),
                storage_path.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        create_ghost_link_windows(&storage_path, &link_path)?;
    }

    Ok(())
}

/// Windows-specific link creation with junction fallback for directories.
#[cfg(windows)]
fn create_ghost_link_windows(storage_path: &Path, link_path: &Path) -> Result<()> {
    if storage_path.is_dir() {
        // Try symlink first; fall back to junction if permission denied
        match std::os::windows::fs::symlink_dir(storage_path, link_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "Warning: Symlink creation failed (need Developer Mode). Using junction instead."
                );
                junction::create(storage_path, link_path).with_context(|| {
                    format!(
                        "failed to create junction {} -> {}",
                        link_path.display(),
                        storage_path.display()
                    )
                })?;
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "failed to create directory symlink {} -> {}",
                        link_path.display(),
                        storage_path.display()
                    )
                });
            }
        }
    } else {
        std::os::windows::fs::symlink_file(storage_path, link_path).with_context(|| {
            format!(
                "failed to create file symlink {} -> {} (file symlinks require Developer Mode on Windows)",
                link_path.display(),
                storage_path.display()
            )
        })?;
    }
    Ok(())
}

/// Remove the symlink (or junction on Windows) at the original location.
pub fn remove_ghost_link(root: &Path, target: &str) -> Result<()> {
    let link_path = root.join(target);

    let meta = link_path.symlink_metadata().with_context(|| {
        format!("symlink does not exist: {}", link_path.display())
    })?;

    if !meta.file_type().is_symlink() {
        // On Windows, check if it's a junction before rejecting
        #[cfg(windows)]
        {
            if junction::exists(&link_path).unwrap_or(false) {
                junction::delete(&link_path).with_context(|| {
                    format!("failed to remove junction: {}", link_path.display())
                })?;
                return Ok(());
            }
        }

        bail!(
            "path is not a symlink (refusing to remove): {}",
            link_path.display()
        );
    }

    // On Unix, symlinks (even to directories) are removed with remove_file.
    // On Windows, directory symlinks need remove_dir.
    #[cfg(unix)]
    {
        std::fs::remove_file(&link_path)
            .with_context(|| format!("failed to remove symlink: {}", link_path.display()))?;
    }

    #[cfg(windows)]
    {
        if meta.is_dir() {
            std::fs::remove_dir(&link_path)
                .with_context(|| format!("failed to remove dir symlink: {}", link_path.display()))?;
        } else {
            std::fs::remove_file(&link_path)
                .with_context(|| format!("failed to remove file symlink: {}", link_path.display()))?;
        }
    }

    Ok(())
}
