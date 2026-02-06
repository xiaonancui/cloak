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
        if storage_path.is_dir() {
            std::os::windows::fs::symlink_dir(&storage_path, &link_path).with_context(|| {
                format!(
                    "failed to create directory symlink {} -> {}",
                    link_path.display(),
                    storage_path.display()
                )
            })?;
        } else {
            std::os::windows::fs::symlink_file(&storage_path, &link_path).with_context(|| {
                format!(
                    "failed to create file symlink {} -> {}",
                    link_path.display(),
                    storage_path.display()
                )
            })?;
        }
    }

    Ok(())
}

/// Remove the symlink at the original location.
pub fn remove_ghost_link(root: &Path, target: &str) -> Result<()> {
    let link_path = root.join(target);

    let meta = link_path.symlink_metadata().with_context(|| {
        format!("symlink does not exist: {}", link_path.display())
    })?;

    if !meta.file_type().is_symlink() {
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
