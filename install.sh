#!/usr/bin/env bash
set -euo pipefail

# Repo root. Derive from this script's location; allow env override.
DOTCONFIG_DIR="${DOTCONFIG_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"

usage() {
    cat <<EOF
Usage: ./install.sh [-h|--help]

Fresh-machine bootstrap. Idempotent — safe to re-run.

Steps:
  0. Linux: install Homebrew's system prerequisites (compiler, curl, file, git, unzip) via apt/dnf/pacman if missing
  1. Install Homebrew (if missing) and load 'brew shellenv' for this session
  2. scripts/brew-preflight.sh --apply — trust Brewfile taps, then 'brew bundle' (fails hard)
  3. Rust: install rustup via sh.rustup.rs (if missing), 'rustup update', add rust-analyzer
  4. Verify nushell is on PATH
  5. Bootstrap sfw (Socket Firewall) via 'bun add --global sfw' (if missing)
  6. just regen — generate shell configs + bin/ scripts and stow into \$HOME
  7. just cargo-install — bootstrap cargo-binstall + cargo-liner, link config, 'cargo liner ship'
  8. just node-install — bun add --global from config/node/package.json
  9. just uv-install — uv tool install from config/uv/tools.txt

Set BREW_TRUST_NEW_TAPS=1 to trust new Brewfile taps without the prompt.
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

# Prepend a directory to PATH for this session (no-op if already present).
path_prepend() {
    case ":$PATH:" in
        *":$1:"*) ;;
        *) export PATH="$1:$PATH" ;;
    esac
}

# Detect OS
OS="$(uname -s)"
ARCH="$(uname -m)"

log_info "Detected OS: $OS ($ARCH)"

# Locate brew even when it is installed but not yet on PATH (fresh shell right
# after the Homebrew installer, which only prints PATH instructions).
find_brew() {
    if command -v brew &> /dev/null; then
        command -v brew
        return 0
    fi
    local candidates
    if [[ "$OS" == "Darwin" ]]; then
        candidates="/opt/homebrew/bin/brew /usr/local/bin/brew"
    else
        candidates="/home/linuxbrew/.linuxbrew/bin/brew $HOME/.linuxbrew/bin/brew"
    fi
    local c
    for c in $candidates; do
        if [[ -x "$c" ]]; then
            echo "$c"
            return 0
        fi
    done
    return 1
}

# Step 0 (Linux): Homebrew's system prerequisites. Linuxbrew refuses every
# install without a system compiler ("No developer tools installed"), and
# unpacking zip casks needs a system unzip before brew pours its own.
install_linux_prereqs() {
    local missing="" cmd
    for cmd in gcc make curl file git unzip ps; do
        command -v "$cmd" &> /dev/null || missing="$missing $cmd"
    done
    [[ -z "$missing" ]] && return 0
    log_info "Installing Homebrew prerequisites (missing:$missing)..."
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo DEBIAN_FRONTEND=noninteractive apt-get install -y build-essential procps curl file git unzip
    elif command -v dnf &> /dev/null; then
        sudo dnf group install -y development-tools
        sudo dnf install -y procps-ng curl file git unzip
    elif command -v pacman &> /dev/null; then
        sudo pacman -S --needed --noconfirm base-devel procps-ng curl file git unzip
    else
        log_error "No apt-get/dnf/pacman found; install a C compiler, make, curl, file, git, unzip and procps, then re-run ./install.sh"
        exit 1
    fi
}
if [[ "$OS" == "Linux" ]]; then
    install_linux_prereqs
fi

# Step 1: Install Homebrew (macOS/Linux) and load its environment
if BREW_BIN="$(find_brew)"; then
    log_info "Homebrew already installed ($BREW_BIN)"
else
    log_info "Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    if ! BREW_BIN="$(find_brew)"; then
        log_error "Homebrew install finished but brew was not found"
        exit 1
    fi
fi
# Add Homebrew to PATH for this session
eval "$("$BREW_BIN" shellenv)"

# Step 2: Install packages via Brewfile. Call the preflight script directly:
# `just` itself is installed by the Brewfile, so it may not exist yet.
log_info "Installing packages from Brewfile..."
if ! "$DOTCONFIG_DIR/scripts/brew-preflight.sh" --apply; then
    log_error "Brewfile install failed (scripts/brew-preflight.sh --apply). Fix the errors above and re-run ./install.sh"
    exit 1
fi

# Step 3: Rust toolchain. rustup is not in the Brewfile; use the official
# installer. --no-modify-path: PATH is managed by the generated shell configs.
CARGO_ENV="${CARGO_HOME:-$HOME/.cargo}/env"
if ! command -v rustup &> /dev/null && [[ -f "$CARGO_ENV" ]]; then
    # shellcheck source=/dev/null
    . "$CARGO_ENV"
fi
if command -v rustup &> /dev/null; then
    log_info "Updating Rust toolchain..."
    rustup update
else
    log_info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    # shellcheck source=/dev/null
    . "$CARGO_ENV"
fi
# rust-analyzer is a rustup component, not a brew package. Without this the
# ~/.cargo/bin/rust-analyzer shim exists but fails with "Unknown binary".
rustup component add rust-analyzer || log_warn "Failed to add rust-analyzer component"

# Step 4: Ensure Nushell is available
if ! command -v nu &> /dev/null; then
    log_error "Nushell not found. Please install via brew install nushell"
    exit 1
fi

# Step 5: Bootstrap sfw (Socket Firewall). The cargo/node/uv install recipes
# run their package managers through sfw, so it must be on PATH first.
# This one install is unavoidably unproxied: sfw cannot vet its own download.
BUN_BIN_DIR="${BUN_INSTALL:-$HOME/.bun}/bin"
path_prepend "$BUN_BIN_DIR"
if ! command -v sfw &> /dev/null; then
    if ! command -v bun &> /dev/null; then
        log_error "bun not found (expected from the Brewfile); cannot bootstrap sfw"
        exit 1
    fi
    log_info "Installing sfw (Socket Firewall) via bun..."
    bun add --global sfw
    if ! command -v sfw &> /dev/null; then
        log_error "sfw installed but not on PATH (looked in $BUN_BIN_DIR)"
        exit 1
    fi
fi

# Step 6: Generate shell configs + stow them
log_info "Generating + stowing shell configurations..."
(cd "$DOTCONFIG_DIR" && just regen)

# Step 7: Install global cargo packages (recipe bootstraps cargo-binstall + cargo-liner,
# links the liner config, then runs 'cargo liner ship').
log_info "Installing global Cargo packages via cargo-liner..."
(cd "$DOTCONFIG_DIR" && just cargo-install) || log_warn "Some cargo packages failed to install"

# Step 8: Install global node packages via bun (installed by Brewfile in step 2)
log_info "Installing global node packages..."
(cd "$DOTCONFIG_DIR" && just node-install) || log_warn "Failed to install some global node packages"

# Step 9: Install global uv (Python) tools from config/uv/tools.txt
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
