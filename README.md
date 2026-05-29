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
3. Generates shell configs and `bin/` scripts from `config/` into `output/`
4. Symlinks `output/` and hand-written packages into `$HOME` via GNU stow
5. Bootstraps `cargo-binstall` + `cargo-liner` and installs global cargo tools from `config/cargo/liner.toml`
6. Installs global npm/bun packages from `config/node/global-packages.json`

After install, run `just doctor` to verify everything is wired correctly.

### One-liner (from anywhere)

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/yurikrupnik/dotconfig/main/bootstrap.sh)
```

`bootstrap.sh` clones the repo to `~/dotconfig` and runs `./install.sh`.

## Daily Commands

```bash
u                           # Refresh installed packages (brew + rust + cargo + node + gcloud) and restow
                            #   — alias for `up`; both resolve to ~/.local/bin/up on every shell

just                        # List all recipes
just doctor                 # Verify install health (symlinks, freshness, deps)
just outdated               # Preview what `u` would refresh

just regen                  # Regenerate output/ from config/ + restow
just stow / unstow          # Re-apply or remove stowed symlinks
just stow-dry               # Preview stow operations
```

`u`/`up` is the daily refresher; `./install.sh` is for fresh machines.

## What Gets Managed

| Component | Source of truth | Destination |
|-----------|-----------------|-------------|
| Shell aliases / env vars / sequence-of-command functions | [`config/shell/config.toml`](config/shell/README.md) | Generated per-shell + `~/.local/bin/<name>` |
| Hand-written scripts (any language) | [`config/scripts/`](config/scripts/README.md) | `~/.local/bin/<name>` |
| Brew packages | `config/brew/Brewfile` | System |
| Cargo tools | `config/cargo/liner.toml` | `~/.cargo/bin/` (via [cargo-liner](https://docs.rs/cargo-liner), symlinked from `$CARGO_HOME/liner.toml`) |
| npm/bun globals | `config/node/global-packages.json` | Global node modules |
| zsh config (hand) | `zsh/` | `~/.zshenv`, `~/.config/zsh/.zshrc` |
| zsh config (generated) | `output/zsh/` | `~/.config/zsh/generated.zsh` |
| nushell config (hand) | `nushell/` | `~/.config/nushell/config.nu`, `env.nu` |
| nushell config (generated) | `output/nu/` | `~/.config/nushell/generated.nu` |
| starship prompt | `starship/` | `~/.config/starship/` |
| zed editor | `zed/` | `~/.config/zed/` |
| zellij layouts | `zellij/layouts/` | (loaded by zellij directly) |

## How It Works

The repo is organized into three roles:

1. **`config/`** — source for things that get *generated*: shell aliases, env vars, sequence-of-command functions (`config/shell/config.toml`), and hand-written scripts in any language (`config/scripts/`).
2. **`output/`** — generator output. **Do not edit by hand. Not committed to git** — rebuilt from `config/` by `shells.nu generate`.
3. **Top-level hand-written stow packages** — `zsh/`, `nushell/`, `zellij/`, `zed/`, `starship/`. These hold config files that are pure source (no generation step). They're committed to git and stowed as-is.

The pipeline:

1. `shells.nu generate` reads `config/shell/config.toml` and `config/scripts/*` and writes everything to `output/`. It also prunes stale entries — if you delete a `[functions.X]` block or a script, the matching executable in `output/bin/` and the dangling symlink in `~/.local/bin/` are removed automatically.
2. `shells.nu stow` uses GNU stow with `--no-folding` to symlink:
   - each subdir of `output/` (generated packages: `bin`, `zsh`, `nu`)
   - each entry of `HAND_WRITTEN_PACKAGES` at the top of the repo (currently `zellij`, `zed`, `starship`, `zsh`, `nushell`)
   …into `$HOME`. Hand-written and generated packages happily share target directories (e.g. `~/.config/zsh/` ends up with `.zshrc` linked from `zsh/` and `generated.zsh` linked from `output/zsh/`).
3. `config/brew/Brewfile`, `config/cargo/liner.toml`, and `config/node/global-packages.json` declare packages installed by `./install.sh` and refreshed by `u`/`up`.

**On a fresh clone**, `output/` does not exist. `./install.sh` runs `generate` before `stow`, so it bootstraps correctly. If you ever run `just stow` directly on a fresh clone, you'll see an error pointing at `just generate`.

## Adding new functionality

This repo has **two places** to define a new command: pick the one that fits.

| You want… | Put it in… | Why |
|---|---|---|
| An alias (`k = "kubectl"`) | `config/shell/config.toml` `[aliases]` | One-liner per shell, no logic |
| An env var (`EDITOR = "zed"`) | `config/shell/config.toml` `[environment]` | Exported in every shell |
| "Run these N commands in order" | `config/shell/config.toml` `[functions.X]` | Emits a bash script on `PATH`. See [`config/shell/README.md`](config/shell/README.md). |
| Anything with flags, branches, loops, or structured data | `config/scripts/<name>.<ext>` | Hand-written file in any language. See [`config/scripts/README.md`](config/scripts/README.md). |

The shell user types `<name>` and the shell finds the resulting file on `PATH` — it doesn't care whether your function came from a TOML block or a hand-written nu script.

### Adding packages

```bash
# Homebrew — add to config/brew/Brewfile
echo 'brew "neovim"' >> config/brew/Brewfile
brew bundle --file=config/brew/Brewfile

# Cargo — add to config/cargo/liner.toml under [packages]
cargo liner ship

# npm/bun — add to config/node/global-packages.json
cd config/node && bun install --global
```

## Testing changes safely

`./install.sh` is idempotent — re-running is the simplest test. For full isolation:

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
├── justfile                            # Task runner (wraps the scripts below)
├── .editorconfig                       # Cross-editor formatting
├── config/                             # Source of truth (hand-edited)
│   ├── brew/Brewfile                   # Homebrew packages
│   ├── cargo/liner.toml                # Global cargo tools
│   ├── node/global-packages.json       # Global npm/bun packages
│   ├── shell/
│   │   ├── config.toml                 # Aliases / env / sequence-of-command functions
│   │   └── README.md                   # When to use [functions.X]
│   └── scripts/                        # Hand-written scripts (any language)
│       ├── mcp.nu                      # → ~/.local/bin/mcp
│       ├── nx-run.nu                   # → ~/.local/bin/nx-run
│       ├── upkg.nu                     # → ~/.local/bin/upkg
│       └── README.md                   # When to write a script; bash vs nu
├── scripts/
│   ├── doctor.sh                       # Health check (commands, symlinks, freshness)
│   ├── outdated.sh                     # Preview pending updates (brew/rust/node)
│   └── nu/setup-local-machine/
│       └── shells.nu                   # Generator + stow driver
├── output/                             # Generated; NOT committed to git
│   ├── bin/.local/bin/                 # Functions from TOML + scripts from config/scripts/
│   ├── zsh/.config/zsh/generated.zsh   # Generated zsh aliases + env
│   └── nu/.config/nushell/generated.nu # Generated nu aliases + env
├── zsh/                                # Hand-written zsh source
│   ├── .zshenv
│   └── .config/zsh/.zshrc
├── nushell/.config/nushell/            # Hand-written nu source
│   ├── config.nu
│   └── env.nu
├── starship/.config/starship/          # Hand-written starship prompt
├── zed/.config/zed/                    # Hand-written Zed config
└── zellij/layouts/                     # Zellij terminal layouts
```

## Command Runners

- **`./install.sh`, `./bootstrap.sh`** — pure bash, run before nu exists
- **`u`/`up`** — daily refresh of installed packages (defined in `config.toml`, lives on `PATH`)
- **`./scripts/doctor.sh`, `./scripts/outdated.sh`** — health check + update preview
- **`just <recipe>`** — short aliases for daily commands; requires `brew install just`

There is no Makefile — the bash scripts are the canonical entry points; `just` is just for ergonomics.

## Supported Platforms

- macOS (Apple Silicon and Intel)
- Linux (via Homebrew on Linux)
