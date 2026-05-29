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

# Update brew packages from Brewfile
brew-install:
    brew bundle --file=config/brew/Brewfile

# Install/update global cargo packages via cargo-liner
cargo-install:
    cargo liner ship --no-fail-fast

# Install global node packages from config/node/package.json
node-install:
    cd config/node && bun install --global

# Verify the install is healthy (commands, symlinks, freshness)
doctor:
    ./scripts/doctor.sh

# Preview what `u` would refresh (read-only)
outdated:
    ./scripts/outdated.sh
