# dotconfig

Personal dotfiles and machine setup. One source of truth for shell configs, packages, and tooling — applied via GNU stow.

## Quick Start

### Fresh Machine

```bash
git clone https://github.com/yurikrupnik/dotconfig.git ~/dotconfig
cd ~/dotconfig
./install.sh
```

This:
1. Installs Homebrew (if missing) and all packages from `config/brew/Brewfile`
2. Updates the Rust toolchain
3. Generates shell configs from `config/shell/config.toml` into `output/`
4. Symlinks `output/` into `$HOME` via GNU stow
5. Bootstraps `cargo-binstall` + `cargo-liner` and installs global cargo tools from `config/cargo/liner.toml`
6. Installs global npm/bun packages from `config/node/global-packages.json`
7. Runs optional cloud-tool setup (`scripts/nu/setup-local-machine/index.nu`)

After install, run `just doctor` to verify everything is wired correctly.

### One-liner (from anywhere)

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/yurikrupnik/dotconfig/main/bootstrap.sh)
```

`bootstrap.sh` clones the repo to `~/dotconfig` and runs `./install.sh`.

### Existing Machine (migration)

If you already have dotfiles symlinked elsewhere, remove the old symlinks first so stow doesn't conflict:

```bash
rm -rf ~/.zshenv ~/.config/{zsh,fish,nushell,bash,starship}
./install.sh
```

Use `./backup.sh` first if you want a snapshot of your current dotfiles.

## Daily Commands

```bash
u                           # Refresh installed packages (brew + rust + cargo + node + gcloud) and restow shells
                            #   — shell function from config.toml, available in zsh/fish/nu/bash

just                        # List all recipes
just doctor                 # Verify install health (symlinks, freshness, deps)
just outdated               # Preview what `u` would refresh

just regen                  # Regenerate shell configs from config.toml + stow
just stow / unstow          # Re-apply or remove stowed symlinks
just stow-dry               # Preview stow operations
just backup                 # Snapshot current dotfiles to ~/dotconfig-backup-<ts>/
```

The `just` recipes are thin wrappers around bash scripts and `nu shells.nu` commands — see [`justfile`](justfile). `u` is the daily refresher; `./install.sh` is for fresh machines.

## What Gets Managed

| Component | Source of truth | Destination |
|-----------|-----------------|-------------|
| Shell aliases / functions / env | `config/shell/config.toml` | Generated per-shell |
| Brew packages | `config/brew/Brewfile` | System |
| Cargo tools | `config/cargo/liner.toml` | `~/.cargo/bin/` (via [cargo-liner](https://docs.rs/cargo-liner), symlinked to `$CARGO_HOME/liner.toml`) |
| npm/bun globals | `config/node/global-packages.json` | Global node modules |
| zsh config | `output/zsh/` | `~/.zshenv`, `~/.config/zsh/` |
| fish config | `output/fish/` | `~/.config/fish/` |
| nushell config | `output/nu/` | `~/.config/nushell/` |
| bash config | `output/bash/` | `~/.config/bash/` |
| starship prompt | `output/starship/` | `~/.config/starship/` |
| zellij layouts | `zellij/layouts/` | (loaded by zellij directly) |

## How It Works

1. **`config.toml`** is the single source of truth for shell aliases, functions, and environment variables.
2. **`shells.nu generate`** reads `config.toml` and writes shell-specific files into `output/<shell>/`.
3. **`shells.nu stow`** uses GNU stow to symlink everything under `output/<shell>/` into `$HOME`.
4. **`config/brew/Brewfile`**, **`config/cargo/liner.toml`**, and **`config/node/global-packages.json`** declare packages installed by `./install.sh` and refreshed by the `u` shell function.

## Customization

### Add a shell alias or function

Edit `config/shell/config.toml`:

```toml
[aliases]
k = "kubectl"
lg = "lazygit"

[functions.greet]
description = "Print a greeting"
command = "echo Hello, $1"
args = ["name"]
```

Then regenerate:

```bash
just regen
```

### Add packages

```bash
# Homebrew — add to config/brew/Brewfile
echo 'brew "neovim"' >> config/brew/Brewfile
brew bundle --file=config/brew/Brewfile

# Cargo — add to config/cargo/liner.toml under [packages]
# Example: cargo-watch = "*"
cargo liner ship

# npm/bun — add to config/node/global-packages.json
cd config/node && bun install --global  # or: npm install --global
```

## Testing changes safely

The best test is just running `./install.sh` — it's idempotent. For full isolation:

```bash
docker run -it --rm -v "$(pwd):/dotconfig" ubuntu:latest bash
# inside the container:
cd /dotconfig && ./install.sh
```

## File Structure

```
dotconfig/
├── install.sh                          # Fresh machine bootstrap
├── bootstrap.sh                        # Clone + install (for curl piping)
├── backup.sh                           # Snapshot current dotfiles
├── justfile                            # Task runner (wraps the above)
├── .editorconfig                       # Cross-editor formatting
├── config/                             # Declarative manifests (sources of truth)
│   ├── brew/Brewfile                   # Homebrew packages
│   ├── cargo/liner.toml                # Global cargo tools (symlinked to $CARGO_HOME/liner.toml)
│   ├── node/global-packages.json       # Global npm/bun packages
│   └── shell/config.toml               # Shell aliases / functions / env
├── scripts/
│   ├── doctor.sh                       # Health check (commands, symlinks, freshness)
│   ├── outdated.sh                     # Preview pending updates (brew/rust/node)
│   └── nu/setup-local-machine/
│       ├── shells.nu                   # Reads shell/config.toml; generates + stows
│       ├── index.nu                    # Optional cloud-tool setup
│       ├── mcp.nu                      # MCP server config generator (optional)
│       ├── nx.nu                       # Nx workspace helpers (optional)
│       └── security.nu                 # Security-related setup (optional)
├── output/                             # Generated shell configs (stowed from here)
│   └── zsh/  fish/  nu/  bash/  starship/  zed/
└── zellij/layouts/                     # Zellij terminal layouts
```

## Command Runners

- **`./install.sh`, `./backup.sh`, `./bootstrap.sh`** — pure bash, always available; all support `-h`/`--help`
- **`u`** — shell function from `config/shell/config.toml`; daily refresh of installed packages
- **`./scripts/doctor.sh`, `./scripts/outdated.sh`** — health check + update preview, no deps
- **`just <recipe>`** — short aliases for daily commands; requires `brew install just`

There is no Makefile — the bash scripts are the canonical entry points; `just` is just for ergonomics.

## Supported Platforms

- macOS (Apple Silicon and Intel)
- Linux (via Homebrew on Linux)
