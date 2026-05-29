#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<EOF
Usage: ./install.sh [-h|--help]

Fresh-machine bootstrap. Idempotent — safe to re-run.

Steps:
  1. Install Homebrew (if missing)
  2. brew bundle --file=config/brew/Brewfile
  3. rustup update
  4. Generate shell configs from config/shell/config.toml
  5. Stow generated configs into \$HOME
  6. Install cargo-binstall, cargo-liner
  7. cargo liner ship (installs config/cargo/liner.toml packages)
  8. bun/npm install --global from config/node/global-packages.json
  9. Run scripts/nu/setup-local-machine/index.nu (optional cloud tools)

After install: run 'just doctor' to verify, then 'u' (from your shell) for periodic refreshes.
EOF
    exit 0
}

case "${1:-}" in
    -h|--help) usage ;;
esac

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}==>${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}==>${NC} $1"
}

log_error() {
    echo -e "${RED}==>${NC} $1"
}

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

log_info "Detected OS: $OS ($ARCH)"

# Step 1: Install Homebrew (macOS/Linux)
if ! command -v brew &> /dev/null; then
    log_info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # Add Homebrew to PATH for this session
    if [[ "$OS" == "Linux" ]]; then
        eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
    fi
else
    log_info "Homebrew already installed"
fi

# Step 2: Install packages via Brewfile
log_info "Installing packages from Brewfile..."
brew bundle --file="$HOME/dotconfig/config/brew/Brewfile" || log_warn "Some brew packages failed to install"

# Step 3: Ensure Rust toolchain is up to date
if command -v rustup &> /dev/null; then
    log_info "Updating Rust toolchain..."
    rustup update
else
    log_error "Rust not found after brew install. Please check Brewfile"
    exit 1
fi

# Step 4: Ensure Nushell is available
if ! command -v nu &> /dev/null; then
    log_error "Nushell not found. Please install via brew install nushell"
    exit 1
fi

# Step 5: Generate shell configurations
log_info "Generating shell configurations..."
cd "$HOME/dotconfig/scripts/nu/setup-local-machine"
nu shells.nu generate

# Step 6: Stow configurations
log_info "Stowing configurations..."
nu shells.nu stow

# Step 7: Install cargo-binstall (for faster binary installations)
if ! command -v cargo-binstall &> /dev/null; then
    log_info "Installing cargo-binstall..."
    cargo install cargo-binstall
fi

# Step 8: Install cargo-liner (declarative manager for global cargo packages)
if ! command -v cargo-liner &> /dev/null; then
    log_info "Installing cargo-liner..."
    cargo binstall cargo-liner --no-confirm
fi

# Step 9: Symlink cargo-liner config and install global cargo packages
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
LINER_SRC="$HOME/dotconfig/config/cargo/liner.toml"
LINER_DEST="$CARGO_HOME_DIR/liner.toml"
if [ -f "$LINER_SRC" ]; then
    if [ ! -L "$LINER_DEST" ] || [ "$(readlink "$LINER_DEST")" != "$LINER_SRC" ]; then
        log_info "Linking cargo-liner config: $LINER_DEST -> $LINER_SRC"
        ln -sfn "$LINER_SRC" "$LINER_DEST"
    fi
    log_info "Installing global Cargo packages via cargo-liner..."
    cargo liner ship --no-fail-fast || log_warn "Some cargo packages failed to install"
fi

# Step 10: Install global npm/bun packages (if global-packages.json exists)
if [ -f "$HOME/dotconfig/config/node/global-packages.json" ]; then
    log_info "Installing global npm/bun packages..."

    if command -v bun &> /dev/null; then
        # Use bun (faster)
        cd "$HOME/dotconfig/config/node"
        bun install --global || log_warn "Failed to install some global packages"
    elif command -v npm &> /dev/null; then
        # Fallback to npm
        cd "$HOME/dotconfig/config/node"
        npm install --global || log_warn "Failed to install some global packages"
    else
        log_warn "Neither bun nor npm found. Skipping global package installation"
    fi
fi

# Step 11: Run additional setup (cloud tools, etc.)
if [ -f "$HOME/dotconfig/scripts/nu/setup-local-machine/index.nu" ]; then
    log_info "Running additional setup tasks..."
    nu "$HOME/dotconfig/scripts/nu/setup-local-machine/index.nu" || log_warn "Some setup tasks failed"
fi

log_info "Installation complete!"
echo ""
log_info "Next steps:"
echo "  1. Restart your shell or run: source ~/.zshenv"
echo "  2. Your generated shell configurations live in: $HOME/dotconfig/output/"
echo "  3. Run 'just doctor' to verify everything is wired correctly"
echo ""
log_warn "Note: Some changes require a full shell restart to take effect"
