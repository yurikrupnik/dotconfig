#!/usr/bin/env bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}==>${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}==>${NC} $1"
}

cd "$HOME/dotconfig"

# Update git repository
log_info "Updating dotconfig repository..."
git pull

# Update Homebrew packages
log_info "Updating Homebrew packages..."
brew update
brew bundle --file="$HOME/dotconfig/brew/Brewfile"
brew upgrade
brew cleanup

# Update Rust toolchain
log_info "Updating Rust toolchain..."
rustup update

# Ensure cargo-binstall is available
if ! command -v cargo-binstall &> /dev/null; then
    log_info "Installing cargo-binstall..."
    cargo install cargo-binstall
fi

# Update global Cargo packages
log_info "Updating global Cargo packages..."
if [ -f "$HOME/dotconfig/cargo-install.toml" ]; then
    cat "$HOME/dotconfig/cargo-install.toml" | grep '^[a-z]' | while read -r pkg; do
        cargo binstall "$pkg" --no-confirm || log_warn "Failed to update $pkg"
    done
fi

# Update global npm/bun packages
if command -v bun &> /dev/null; then
    log_info "Updating global bun packages..."
    bun update --global
elif command -v npm &> /dev/null; then
    log_info "Updating global npm packages..."
    npm update --global
fi

# Update cloud tools
if command -v gcloud &> /dev/null; then
    log_info "Updating gcloud components..."
    gcloud components update --quiet || log_warn "Failed to update gcloud"
fi

# Regenerate shell configurations
log_info "Regenerating shell configurations..."
cd "$HOME/dotconfig/scripts/nu/setup-local-machine"
nu shells.nu generate
nu shells.nu stow

# Rebuild dotconfig CLI
log_info "Rebuilding dotconfig CLI..."
cd "$HOME/dotconfig"
cargo build --release

log_info "Update complete!"
