<p align="center">
  <h1 align="center">Cloak</h1>
  <p align="center"><strong>Config files should work, not be seen.</strong></p>
  <p align="center">Zero-perception config file organizer for vibe coders.</p>
</p>

<p align="center">
  <a href="https://github.com/xiaonancui/cloak/actions"><img src="https://github.com/xiaonancui/cloak/actions/workflows/release.yml/badge.svg" alt="Build"></a>
  <a href="https://github.com/xiaonancui/cloak/releases"><img src="https://img.shields.io/github/v/release/xiaonancui/cloak" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
</p>

---

Every AI coding tool drops a dotfile folder in your project root. Cursor leaves `.cursor/`, Claude Code leaves `.claude/`, Windsurf leaves `.windsurf/` -- before you know it, your clean project root looks like a config junkyard.

**Cloak** moves these config directories into a hidden `.cloak/storage/` vault, replaces them with invisible symlinks, and hides them from your OS file manager and IDE sidebar. Your tools keep working. Your root stays clean.

## How It Works

```
Before                          After
├── .cursor/                    ├── .cursor -> .cloak/storage/.cursor (hidden)
├── .vscode/                    ├── .vscode -> .cloak/storage/.vscode (hidden)
├── .claude/                    ├── .claude -> .cloak/storage/.claude (hidden)
├── .idea/                      ├── .cloak/
├── src/                        │   └── storage/
├── package.json                │       ├── .cursor/
└── ...                         │       ├── .vscode/
                                │       ├── .claude/
                                │       └── .idea/
                                ├── src/
                                ├── package.json
                                └── ...
```

Symlinks are invisible in Finder/Explorer (OS-level hidden flag) and excluded from VS Code / Cursor sidebars (`files.exclude`). The real configs in `.cloak/storage/` can be committed to git.

## Install

### From Releases

Download the latest binary from [GitHub Releases](https://github.com/xiaonancui/cloak/releases) and add it to your `PATH`.

### From Source

```bash
cargo install --path .
```

## Quick Start

```bash
# Auto-scan and hide all known AI tool configs
cloak tidy --yes

# Or hide specific targets
cloak hide .cursor .vscode .claude

# Check what's hidden
cloak status

# Restore when needed
cloak unhide .cursor
```

## Commands

| Command | Description |
|---------|-------------|
| `cloak init` | Initialize cloak in the current project |
| `cloak hide <targets...>` | Hide specified config dirs into `.cloak/storage/` |
| `cloak unhide <targets...>` | Restore hidden configs back to their original locations |
| `cloak tidy [--yes]` | Auto-scan for known AI tool configs and hide them all |
| `cloak status` | Show hidden configs, link health, and orphaned symlinks |

### Global Options

| Option | Description |
|--------|-------------|
| `--root <path>` | Project root directory (defaults to current directory) |

## What `tidy` Detects

Cloak auto-detects config directories from 22 mainstream AI coding tools:

**AI IDEs / Editors:** Cursor, VS Code, Windsurf, Trae, Zed

**JetBrains:** IntelliJ/PyCharm (.idea), Junie

**AI Coding Agents:** Claude Code, OpenAI Codex, Gemini, Amazon Q, Augment, Bolt, Tabnine

**China AI Tools:** CodeBuddy (Tencent), Tongyi Lingma (Alibaba), Comate (Baidu), Kimi Code (Moonshot)

**VS Code Extensions:** Cline, Roo Code, Kilo Code

## The Hide Pipeline

When you run `cloak hide .cursor`:

1. **Move** `.cursor/` into `.cloak/storage/.cursor/`
2. **Symlink** `.cursor` -> `.cloak/storage/.cursor/` (junction fallback on Windows)
3. **OS-hide** the symlink (macOS `chflags hidden`, Windows `FILE_ATTRIBUTE_HIDDEN`)
4. **IDE-exclude** add `**/.cursor` to `.vscode/settings.json` and `.cursor/settings.json` `files.exclude`
5. **Git-ignore** add `/.cursor` to the managed section in `.gitignore`

`cloak unhide` reverses all 5 steps.

## Git Integration

Cloak manages your `.gitignore` with two blocks:

```gitignore
# --- Cloak ---
/.cloak/*
!/.cloak/storage/

# >>> cloak managed
/.cursor
/.vscode
/.claude
# <<< cloak managed
```

- `/.cloak/*` ignores cloak internals
- `!/.cloak/storage/` whitelists the real configs so they can be committed
- The managed section ignores root symlinks (machine-specific)

## Safety

- **Conflict protection:** `unhide` refuses to overwrite if a real file/directory already exists at the target path
- **Orphan detection:** `status` detects and reports dangling symlinks whose storage targets are missing
- **Input validation:** rejects path traversal, absolute paths, and nested targets
- **Cross-device support:** falls back to copy+delete when `rename` fails across filesystems
- **Windows support:** falls back to NTFS junctions when symlinks require Developer Mode

## License

MIT
