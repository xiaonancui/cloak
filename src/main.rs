mod config;
mod core;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::{Path, PathBuf};

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
}

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
    }
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
        println!("{} {}", "Hiding".bold(), target.yellow());

        core::mover::ingest(root, target)?;
        core::linker::create_ghost_link(root, target)?;
        core::hider::hide_path(root, target)?;
        config::ide::add_vscode_exclude(root, target)?;
        utils::git::add_ignore_entry(root, target)?;

        println!("  {} {}", "✓".green(), target);
    }

    println!("{}", "Done. Your root directory is now pristine.".green());
    Ok(())
}

fn cmd_unhide(root: &Path, targets: &[String]) -> Result<()> {
    for target in targets {
        println!("{} {}", "Restoring".bold(), target.yellow());

        config::ide::remove_vscode_exclude(root, target)?;
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
