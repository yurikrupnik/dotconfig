# https://just.systems
# Dotfiles task runner. For details on what each script does, see README.md.

set shell := ["bash", "-cu"]

default:
    @just --list

# Fresh-machine bootstrap (brew, rust, cargo-liner, shells, stow)
install:
    ./install.sh

# Backup existing dotfiles before applying new configs
backup:
    ./backup.sh

# Generate shell configs from config.toml
generate:
    nu scripts/nu/setup-local-machine/shells.nu generate

# Symlink generated configs into $HOME (via GNU stow)
stow:
    nu scripts/nu/setup-local-machine/shells.nu stow

# Remove symlinked configs from $HOME
unstow:
    nu scripts/nu/setup-local-machine/shells.nu unstow

# Generate + stow in one step
regen: generate stow

# Show what stow would do without making changes
stow-dry:
    nu scripts/nu/setup-local-machine/shells.nu stow --dry-run

# Update brew packages from Brewfile
brew-install:
    brew bundle --file=config/brew/Brewfile

# Install/update global cargo packages via cargo-liner
cargo-install:
    cargo liner ship --no-fail-fast

# Verify the install is healthy (commands, symlinks, freshness)
doctor:
    ./scripts/doctor.sh

# Preview what `u` would refresh (read-only)
outdated:
    ./scripts/outdated.sh
