# https://just.systems
# Dotfiles task runner. For details on what each script does, see README.md.

set shell := ["bash", "-cu"]

shells := "nu scripts/nu/setup-local-machine/shells.nu"

default:
    @just --list

# Fresh-machine bootstrap (brew, rust, cargo-liner, shells, stow)
install:
    ./install.sh

# Generate shell configs and bin/ scripts from config/
generate:
    {{shells}} generate

# Symlink generated configs into $HOME (via GNU stow)
stow:
    {{shells}} stow

# Remove symlinked configs from $HOME
unstow:
    {{shells}} unstow

# Generate + stow in one step
regen: generate stow

# Show what stow would do without making changes
stow-dry:
    {{shells}} stow --dry-run

# Preview Brewfile install: counts + which taps need trust (read-only)
brew-preflight:
    ./scripts/brew-preflight.sh --check

# Update brew packages from Brewfile (preflight → trust new taps → bundle)
brew-install:
    ./scripts/brew-preflight.sh --apply

# Install/update global cargo packages via cargo-liner (bootstraps binstall + liner)
cargo-install:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v cargo-binstall &> /dev/null; then
        echo "==> Installing cargo-binstall..."
        cargo install cargo-binstall
    fi
    if ! command -v cargo-liner &> /dev/null; then
        echo "==> Installing cargo-liner..."
        cargo binstall cargo-liner --no-confirm
    fi
    liner_src="{{justfile_directory()}}/config/cargo/liner.toml"
    liner_dest="${CARGO_HOME:-$HOME/.cargo}/liner.toml"
    if [ ! -L "$liner_dest" ] || [ "$(readlink "$liner_dest")" != "$liner_src" ]; then
        echo "==> Linking cargo-liner config: $liner_dest -> $liner_src"
        ln -sfn "$liner_src" "$liner_dest"
    fi
    cargo liner ship --no-fail-fast

# Install/refresh global node packages declared in config/node/package.json
node-install:
    cd config/node && bun add --global $(jq -r '.dependencies | to_entries[] | "\(.key)@\(.value)"' package.json)

# Install/refresh global Python CLI tools declared in config/uv/tools.txt
uv-install:
    awk '!/^[[:space:]]*(#|$)/ {print $1}' config/uv/tools.txt | while IFS= read -r pkg; do uv tool install "$pkg" || echo "  ! uv tool install $pkg failed"; done

# Verify the install is healthy (commands, symlinks, freshness)
doctor:
    ./scripts/doctor.sh

# Preview what `u` would refresh (read-only)
outdated:
    ./scripts/outdated.sh
