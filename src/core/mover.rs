use anyhow::{bail, Context, Result};
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

    fs::rename(&src, &dest).with_context(|| {
        format!(
            "failed to move {} -> {}",
            src.display(),
            dest.display()
        )
    })?;

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

    fs::rename(&src, &dest).with_context(|| {
        format!(
            "failed to move {} -> {}",
            src.display(),
            dest.display()
        )
    })?;

    Ok(())
}
