use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

const SETTINGS_FILE: &str = "settings.json";
const EXCLUDE_KEY: &str = "files.exclude";

/// IDE directories whose `settings.json` we manage.
/// .vscode settings are always created; others only if the directory already exists.
const IDE_DIRS: &[&str] = &[".vscode", ".cursor"];

/// Add a target to `files.exclude` in all relevant IDE settings files.
pub fn add_ide_exclude(root: &Path, target: &str) -> Result<()> {
    let exclude_key = format!("**/{target}");

    for ide_dir in IDE_DIRS {
        let dir_path = root.join(ide_dir);
        let settings_path = dir_path.join(SETTINGS_FILE);

        // For .vscode, always create if needed. For others, only write if the dir exists.
        if *ide_dir != ".vscode" && !dir_path.exists() {
            continue;
        }

        let mut settings = load_or_create_settings(&settings_path)?;

        let exclude = settings
            .entry(EXCLUDE_KEY)
            .or_insert_with(|| Value::Object(Map::new()));

        if let Value::Object(map) = exclude {
            map.insert(exclude_key.clone(), Value::Bool(true));
        }

        save_settings(&settings_path, &settings)?;
    }

    Ok(())
}

/// Remove a target from `files.exclude` in all relevant IDE settings files.
pub fn remove_ide_exclude(root: &Path, target: &str) -> Result<()> {
    let exclude_key = format!("**/{target}");

    for ide_dir in IDE_DIRS {
        let settings_path = root.join(ide_dir).join(SETTINGS_FILE);

        if !settings_path.exists() {
            continue;
        }

        let mut settings = load_or_create_settings(&settings_path)?;

        if let Some(Value::Object(map)) = settings.get_mut(EXCLUDE_KEY) {
            // Remove both the glob-prefixed key and any legacy bare key
            map.remove(&exclude_key);
            map.remove(target);
        }

        save_settings(&settings_path, &settings)?;
    }

    Ok(())
}

fn load_or_create_settings(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    // Strip single-line comments (// ...) and block comments (/* ... */) for JSONC support.
    let stripped = strip_jsonc_comments(&content);

    let value: Value = serde_json::from_str(&stripped)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    match value {
        Value::Object(map) => Ok(map),
        _ => Ok(Map::new()),
    }
}

fn save_settings(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(&Value::Object(settings.clone()))
        .context("failed to serialize settings")?;

    fs::write(path, content.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(())
}

/// Minimal JSONC comment stripper that handles `//` and `/* */` comments
/// while respecting string literals.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inside a string literal
        if chars[i] == '"' {
            out.push(chars[i]);
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            if i < len {
                out.push(chars[i]); // closing quote
                i += 1;
            }
            continue;
        }

        // Line comment
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            // Skip until end of line
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
            continue;
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}
