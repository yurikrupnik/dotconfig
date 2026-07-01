#!/usr/bin/env bash
set -euo pipefail

# Repo root. Derive from this script's location; allow env override.
DOTCONFIG_DIR="${DOTCONFIG_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"

usage() {
    cat <<EOF
Usage: ./install.sh [-h|--help]

Fresh-machine bootstrap. Idempotent — safe to re-run.

Steps:
  1. Install Homebrew (if missing)
  2. brew bundle --file=config/brew/Brewfile
  3. rustup update
  4. Verify nushell is on PATH
  5. just regen — generate shell configs + bin/ scripts and stow into \$HOME
  6. just cargo-install — bootstrap cargo-binstall + cargo-liner, link config, 'cargo liner ship'
  7. bun/npm install --global from config/node/package.json
  8. uv tool install from config/uv/tools.txt

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
(cd "$DOTCONFIG_DIR" && just brew-install) || log_warn "Some brew packages failed to install"

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

# Step 5: Generate shell configs + stow them
log_info "Generating + stowing shell configurations..."
(cd "$DOTCONFIG_DIR" && just regen)

# Step 6: Install global cargo packages (recipe bootstraps cargo-binstall + cargo-liner,
# links the liner config, then runs 'cargo liner ship').
log_info "Installing global Cargo packages via cargo-liner..."
(cd "$DOTCONFIG_DIR" && just cargo-install) || log_warn "Some cargo packages failed to install"

# Step 7: Install global node packages via bun (installed by Brewfile in step 2)
log_info "Installing global node packages..."
(cd "$DOTCONFIG_DIR" && just node-install) || log_warn "Failed to install some global node packages"

# Step 8: Install global uv (Python) tools from config/uv/tools.txt
if command -v uv &> /dev/null; then
    log_info "Installing global uv tools..."
    (cd "$DOTCONFIG_DIR" && just uv-install) || log_warn "Failed to install some uv tools"
else
    log_warn "uv not found; skipping global uv tools install"
fi

log_info "Installation complete!"
echo ""
log_info "Next steps:"
echo "  1. Restart your shell or run: source ~/.zshenv"
echo "  2. Your generated shell configurations live in: $DOTCONFIG_DIR/output/"
echo "  3. Run 'just doctor' to verify everything is wired correctly"
echo ""
log_warn "Note: Some changes require a full shell restart to take effect"
