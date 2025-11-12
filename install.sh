#!/usr/bin/env bash
set -e

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
brew bundle --file="$HOME/dotconfig/brew/Brewfile" || log_warn "Some brew packages failed to install"

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

# Step 8: Install global cargo packages (if cargo-install.toml exists)
if [ -f "$HOME/dotconfig/cargo-install.toml" ]; then
    log_info "Installing global Cargo packages..."
    cat "$HOME/dotconfig/cargo-install.toml" | grep '^[a-z]' | while read -r pkg; do
        cargo binstall "$pkg" --no-confirm || log_warn "Failed to install $pkg"
    done
fi

# Step 9: Install global npm/bun packages (if global-packages.json exists)
if [ -f "$HOME/dotconfig/global-packages.json" ]; then
    log_info "Installing global npm/bun packages..."

    if command -v bun &> /dev/null; then
        # Use bun (faster)
        cd "$HOME/dotconfig"
        bun install --global || log_warn "Failed to install some global packages"
    elif command -v npm &> /dev/null; then
        # Fallback to npm
        cd "$HOME/dotconfig"
        npm install --global || log_warn "Failed to install some global packages"
    else
        log_warn "Neither bun nor npm found. Skipping global package installation"
    fi
fi

# Step 10: Run additional setup (cloud tools, etc.)
if [ -f "$HOME/dotconfig/scripts/nu/setup-local-machine/index.nu" ]; then
    log_info "Running additional setup tasks..."
    nu "$HOME/dotconfig/scripts/nu/setup-local-machine/index.nu" || log_warn "Some setup tasks failed"
fi

# Step 11: Build the dotconfig CLI tool
log_info "Building dotconfig CLI tool..."
cd "$HOME/dotconfig"
cargo build --release

# Step 12: Add to PATH (if not already)
if ! command -v dotconfig &> /dev/null; then
    log_info "Adding dotconfig to PATH..."
    echo 'export PATH="$HOME/dotconfig/target/release:$PATH"' >> "$HOME/.zshrc"
    echo 'export PATH="$HOME/dotconfig/target/release:$PATH"' >> "$HOME/.bashrc"
fi

log_info "Installation complete!"
echo ""
log_info "Next steps:"
echo "  1. Restart your shell or run: source ~/.zshrc (or ~/.bashrc)"
echo "  2. Run 'dotconfig --help' to see available commands"
echo "  3. Your shell configurations are in: $HOME/dotconfig/output/"
echo ""
log_warn "Note: Some changes require a full shell restart to take effect"
