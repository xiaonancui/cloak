use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const GITIGNORE: &str = ".gitignore";
const CLOAK_SECTION_START: &str = "# >>> cloak managed";
const CLOAK_SECTION_END: &str = "# <<< cloak managed";

/// Ensure `.cloak/` is in `.gitignore` (run during `cloak init`).
pub fn ensure_gitignore_entry(root: &Path) -> Result<()> {
    let gitignore_path = root.join(GITIGNORE);
    let mut content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .with_context(|| format!("failed to read {}", gitignore_path.display()))?
    } else {
        String::new()
    };

    // Don't duplicate
    if content.contains(".cloak/") {
        return Ok(());
    }

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }

    content.push_str("\n# Cloak storage\n.cloak/\n");

    fs::write(&gitignore_path, content.as_bytes())
        .with_context(|| format!("failed to write {}", gitignore_path.display()))?;

    Ok(())
}

/// Add a symlink target to the cloak-managed section in `.gitignore`.
pub fn add_ignore_entry(root: &Path, target: &str) -> Result<()> {
    let gitignore_path = root.join(GITIGNORE);
    let content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)
            .with_context(|| format!("failed to read {}", gitignore_path.display()))?
    } else {
        String::new()
    };

    let mut entries = parse_managed_section(&content);

    // Don't duplicate
    if entries.contains(&target.to_string()) {
        return Ok(());
    }

    entries.push(target.to_string());
    let new_content = rebuild_gitignore(&content, &entries);

    fs::write(&gitignore_path, new_content.as_bytes())
        .with_context(|| format!("failed to write {}", gitignore_path.display()))?;

    Ok(())
}

/// Remove a symlink target from the cloak-managed section in `.gitignore`.
pub fn remove_ignore_entry(root: &Path, target: &str) -> Result<()> {
    let gitignore_path = root.join(GITIGNORE);

    if !gitignore_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&gitignore_path)
        .with_context(|| format!("failed to read {}", gitignore_path.display()))?;

    let mut entries = parse_managed_section(&content);
    entries.retain(|e| e != target);

    let new_content = rebuild_gitignore(&content, &entries);

    fs::write(&gitignore_path, new_content.as_bytes())
        .with_context(|| format!("failed to write {}", gitignore_path.display()))?;

    Ok(())
}

/// Extract entries from the `# >>> cloak managed` section.
fn parse_managed_section(content: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.trim() == CLOAK_SECTION_START {
            in_section = true;
            continue;
        }
        if line.trim() == CLOAK_SECTION_END {
            in_section = false;
            continue;
        }
        if in_section {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                entries.push(trimmed.to_string());
            }
        }
    }

    entries
}

/// Rebuild the full `.gitignore` content, replacing the managed section.
fn rebuild_gitignore(content: &str, entries: &[String]) -> String {
    let mut out = String::new();
    let mut in_section = false;
    let mut section_found = false;

    for line in content.lines() {
        if line.trim() == CLOAK_SECTION_START {
            in_section = true;
            section_found = true;
            continue;
        }
        if line.trim() == CLOAK_SECTION_END {
            in_section = false;
            continue;
        }
        if !in_section {
            out.push_str(line);
            out.push('\n');
        }
    }

    // Append managed section if there are entries
    if !entries.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(CLOAK_SECTION_START);
        out.push('\n');
        for entry in entries {
            out.push_str(entry);
            out.push('\n');
        }
        out.push_str(CLOAK_SECTION_END);
        out.push('\n');
    } else if section_found {
        // Section existed but is now empty — already stripped above, nothing to add back.
    }

    out
}
