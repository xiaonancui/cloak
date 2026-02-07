mod config;
mod core;
mod utils;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cloak",
    about = "Config files should work, not be seen.",
    long_about = "Cloak hides dotfiles and config directories from your project root \
                  while keeping them fully functional via symlinks.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Project root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize cloak in the current project
    Init,

    /// Hide specified config files/directories into .cloak/storage
    Hide {
        /// Config paths to hide (e.g. .cursor .vscode .idea)
        #[arg(required = true)]
        targets: Vec<String>,
    },

    /// Restore hidden configs back to their original locations
    Unhide {
        /// Config paths to restore (e.g. .cursor .vscode)
        #[arg(required = true)]
        targets: Vec<String>,
    },

    /// Show current cloak status and managed items
    Status,

    /// Auto-scan project root for common dotfiles and hide them all
    Tidy {
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}

/// Known vibe coding tool config directories to auto-detect with `tidy`.
const KNOWN_DOTFILES: &[&str] = &[
    // AI IDEs / Editors
    ".cursor",
    ".vscode",
    ".windsurf",
    ".trae",
    ".zed",
    // JetBrains
    ".idea",
    ".junie",
    // AI coding agents
    ".claude",
    ".codex",
    ".gemini",
    ".amazonq",
    ".augment",
    ".bolt",
    ".tabnine",
    // China AI coding tools (中国大模型代码工具)
    ".codebuddy",
    ".lingma",
    ".comate",
    ".kimi",
    // VS Code AI extensions
    ".cline",
    ".roo",
    ".kilocode",
];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli
        .root
        .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    match cli.command {
        Commands::Init => cmd_init(&root),
        Commands::Hide { targets } => cmd_hide(&root, &targets),
        Commands::Unhide { targets } => cmd_unhide(&root, &targets),
        Commands::Status => cmd_status(&root),
        Commands::Tidy { yes } => cmd_tidy(&root, yes),
    }
}

/// Validate a target name before hiding.
fn validate_target(target: &str) -> Result<()> {
    if target.is_empty() {
        bail!("target name cannot be empty");
    }

    if target.starts_with('/') || target.starts_with('\\') {
        bail!("absolute paths are not allowed: {target}");
    }

    // Reject Windows-style absolute paths like C:\foo
    if target.len() >= 2 && target.as_bytes()[1] == b':' {
        bail!("absolute paths are not allowed: {target}");
    }

    if target == ".." || target.contains("/../") || target.starts_with("../") || target.ends_with("/..") {
        bail!("path traversal is not allowed: {target}");
    }

    if target == ".cloak" || target.starts_with(".cloak/") || target.starts_with(".cloak\\") {
        bail!("cannot hide the .cloak directory itself");
    }

    if target.contains('/') || target.contains('\\') {
        bail!("only top-level entries are allowed (no path separators): {target}");
    }

    Ok(())
}

/// Ensure cloak is initialized, auto-initializing if needed.
fn ensure_initialized(root: &Path) -> Result<()> {
    let storage = root.join(".cloak").join("storage");
    if !storage.exists() {
        println!("{}", "Auto-initializing cloak...".dimmed());
        core::mover::ensure_storage_dir(root)?;
        utils::git::ensure_gitignore_entry(root)?;
    }
    Ok(())
}

fn cmd_init(root: &Path) -> Result<()> {
    println!("{}", "Initializing cloak...".bold());

    core::mover::ensure_storage_dir(root)?;
    utils::git::ensure_gitignore_entry(root)?;

    println!(
        "{}",
        "Cloak initialized. Use `cloak hide <target>` to start hiding configs.".green()
    );
    Ok(())
}

fn cmd_hide(root: &Path, targets: &[String]) -> Result<()> {
    for target in targets {
        validate_target(target)?;
    }

    ensure_initialized(root)?;

    for target in targets {
        println!("{} {}", "Hiding".bold(), target.yellow());

        core::mover::ingest(root, target)?;
        core::linker::create_ghost_link(root, target)?;
        core::hider::hide_path(root, target)?;
        config::ide::add_ide_exclude(root, target)?;
        utils::git::add_ignore_entry(root, target)?;

        println!("  {} {}", "✓".green(), target);
    }

    println!("{}", "Done. Your root directory is now pristine.".green());
    Ok(())
}

fn cmd_unhide(root: &Path, targets: &[String]) -> Result<()> {
    for target in targets {
        println!("{} {}", "Restoring".bold(), target.yellow());

        config::ide::remove_ide_exclude(root, target)?;
        utils::git::remove_ignore_entry(root, target)?;
        core::hider::unhide_path(root, target)?;
        core::linker::remove_ghost_link(root, target)?;
        core::mover::egest(root, target)?;

        println!("  {} {}", "✓".green(), target);
    }

    println!(
        "{}",
        "Done. Configs restored to their original locations.".green()
    );
    Ok(())
}

fn cmd_status(root: &Path) -> Result<()> {
    let storage = root.join(".cloak").join("storage");

    if !storage.exists() {
        println!(
            "{}",
            "Cloak is not initialized in this directory. Run `cloak init` first.".yellow()
        );
        return Ok(());
    }

    let entries: Vec<_> = std::fs::read_dir(&storage)?
        .filter_map(|e| e.ok())
        .collect();

    if entries.is_empty() {
        println!("{}", "No configs are currently hidden.".dimmed());
        return Ok(());
    }

    println!("{}", "Hidden configs:".bold());
    for entry in entries {
        let name = entry.file_name();
        let link_path = root.join(&name);
        let link_ok = link_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        let status = if link_ok {
            "linked".green()
        } else {
            "link missing".red()
        };

        println!("  {} [{}]", name.to_string_lossy(), status);
    }

    Ok(())
}

fn cmd_tidy(root: &Path, skip_confirm: bool) -> Result<()> {
    ensure_initialized(root)?;

    let storage = root.join(".cloak").join("storage");

    // Scan root for known dotfiles that exist and aren't already hidden
    let mut discovered: Vec<&str> = Vec::new();
    for pattern in KNOWN_DOTFILES {
        let path = root.join(pattern);
        let already_hidden = storage.join(pattern).exists();

        // Skip if already hidden or doesn't exist at root
        if already_hidden {
            continue;
        }

        // Check if it exists as a real file/dir (not a symlink pointing to storage)
        if path.exists() {
            // If it's a symlink to our storage, skip it
            if let Ok(meta) = path.symlink_metadata()
                && meta.file_type().is_symlink()
            {
                continue;
            }
            discovered.push(pattern);
        }
    }

    if discovered.is_empty() {
        println!("{}", "No known dotfiles/configs found to hide.".dimmed());
        return Ok(());
    }

    println!("{}", "Discovered configs:".bold());
    for name in &discovered {
        println!("  {}", name.yellow());
    }

    if !skip_confirm {
        print!("\nHide all {} items? [y/N] ", discovered.len());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("{}", "Aborted.".dimmed());
            return Ok(());
        }
    }

    println!();
    let targets: Vec<String> = discovered.iter().map(|s| s.to_string()).collect();
    for target in &targets {
        println!("{} {}", "Hiding".bold(), target.yellow());

        core::mover::ingest(root, target)?;
        core::linker::create_ghost_link(root, target)?;
        core::hider::hide_path(root, target)?;
        config::ide::add_ide_exclude(root, target)?;
        utils::git::add_ignore_entry(root, target)?;

        println!("  {} {}", "✓".green(), target);
    }

    println!(
        "{}",
        format!("Done. {} configs hidden.", targets.len()).green()
    );
    Ok(())
}
