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

    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(prefix: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let pid = std::process::id();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        dir.push(format!("cloak-{prefix}-{pid}-{nanos}-{seq}"));
        fs::create_dir_all(&dir).expect("failed to create temp test dir");
        dir
    }

    #[test]
    fn strip_jsonc_comments_keeps_comment_like_text_inside_strings() {
        let input = r#"{
  // comment
  "url": "https://example.com/a/*b*/c",
  "v": 1 /* trailing block */
}"#;
        let stripped = strip_jsonc_comments(input);
        let parsed: Value = serde_json::from_str(&stripped).expect("json parse failed");
        assert_eq!(parsed["url"], "https://example.com/a/*b*/c");
        assert_eq!(parsed["v"], 1);
    }

    #[test]
    fn add_and_remove_ide_exclude_round_trip() {
        let root = make_temp_dir("ide-roundtrip");

        let vscode = root.join(".vscode");
        let cursor = root.join(".cursor");
        fs::create_dir_all(&vscode).expect("create .vscode failed");
        fs::create_dir_all(&cursor).expect("create .cursor failed");

        fs::write(
            vscode.join("settings.json"),
            "{\n  \"editor.tabSize\": 2\n}\n",
        )
        .expect("write vscode settings failed");
        fs::write(
            cursor.join("settings.json"),
            "{\n  // comment\n  \"foo\": 1\n}\n",
        )
        .expect("write cursor settings failed");

        add_ide_exclude(&root, ".cursor").expect("add_ide_exclude failed");

        let vscode_json: Value = serde_json::from_str(
            &fs::read_to_string(vscode.join("settings.json")).expect("read vscode settings failed"),
        )
        .expect("parse vscode settings failed");
        assert_eq!(vscode_json["files.exclude"]["**/.cursor"], true);

        let cursor_json: Value = serde_json::from_str(
            &fs::read_to_string(cursor.join("settings.json")).expect("read cursor settings failed"),
        )
        .expect("parse cursor settings failed");
        assert_eq!(cursor_json["files.exclude"]["**/.cursor"], true);

        remove_ide_exclude(&root, ".cursor").expect("remove_ide_exclude failed");
        let vscode_after: Value = serde_json::from_str(
            &fs::read_to_string(vscode.join("settings.json")).expect("read vscode settings failed"),
        )
        .expect("parse vscode settings failed");
        assert!(vscode_after["files.exclude"]["**/.cursor"].is_null());

        fs::remove_dir_all(root).expect("cleanup failed");
    }
}
