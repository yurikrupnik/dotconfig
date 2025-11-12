#!/usr/bin/env bash
# Test installation script - validates without installing
set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}✓${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}!${NC} $1"
}

log_test() {
    echo -e "${BLUE}→${NC} $1"
}

echo "=== Dotconfig Installation Test ==="
echo ""

# Test 1: Check if required files exist
log_test "Checking required files..."
REQUIRED_FILES=(
    "install.sh"
    "update.sh"
    "bootstrap.sh"
    "Makefile"
    "brew/Brewfile"
    "cargo-install.toml"
    "global-packages.json"
    "scripts/nu/setup-local-machine/shells.nu"
    "scripts/nu/setup-local-machine/config.toml"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        log_info "$file exists"
    else
        log_error "$file missing"
    fi
done
echo ""

# Test 2: Check script permissions
log_test "Checking script permissions..."
SCRIPTS=("install.sh" "update.sh" "bootstrap.sh")
for script in "${SCRIPTS[@]}"; do
    if [ -x "$script" ]; then
        log_info "$script is executable"
    else
        log_error "$script is not executable (run: chmod +x $script)"
    fi
done
echo ""

# Test 3: Validate Brewfile syntax
log_test "Validating Brewfile..."
if brew bundle check --file=brew/Brewfile &>/dev/null; then
    log_info "Brewfile syntax is valid"
else
    log_warn "Brewfile check failed (this is ok if packages aren't installed)"
fi
echo ""

# Test 4: Validate cargo-install.toml
log_test "Validating cargo-install.toml..."
PACKAGE_COUNT=$(grep '^[a-z]' cargo-install.toml | wc -l | tr -d ' ')
log_info "Found $PACKAGE_COUNT Cargo packages to install"
echo "Packages:"
grep '^[a-z]' cargo-install.toml | sed 's/^/  - /'
echo ""

# Test 5: Validate global-packages.json
log_test "Validating global-packages.json..."
if command -v jq &>/dev/null; then
    if jq empty global-packages.json 2>/dev/null; then
        log_info "global-packages.json is valid JSON"
        NPM_PACKAGE_COUNT=$(jq '.dependencies | length' global-packages.json)
        log_info "Found $NPM_PACKAGE_COUNT npm/bun packages to install"
    else
        log_error "global-packages.json has invalid JSON syntax"
    fi
else
    log_warn "jq not installed, skipping JSON validation"
fi
echo ""

# Test 6: Check if Nushell scripts are valid
log_test "Validating Nushell scripts..."
if command -v nu &>/dev/null; then
    if nu -c "use scripts/nu/setup-local-machine/shells.nu" 2>/dev/null; then
        log_info "shells.nu is valid"
    else
        log_warn "shells.nu validation failed"
    fi

    if [ -f "scripts/nu/setup-local-machine/config.toml" ]; then
        log_info "config.toml exists"
    fi
else
    log_warn "Nushell not installed, skipping Nu script validation"
fi
echo ""

# Test 7: Simulate what would be installed
log_test "Simulation summary..."
echo ""
echo "The install.sh script would:"
echo "  1. Install/update Homebrew"
echo "  2. Install $(brew bundle list --file=brew/Brewfile 2>/dev/null | wc -l | tr -d ' ') packages from Brewfile"
echo "  3. Update Rust toolchain"
echo "  4. Install cargo-binstall"
echo "  5. Install $PACKAGE_COUNT Cargo packages"
if [ -f "global-packages.json" ]; then
    echo "  6. Install npm/bun packages from global-packages.json"
fi
echo "  7. Generate shell configurations"
echo "  8. Stow configurations to home directory"
echo "  9. Build dotconfig CLI tool"
echo ""

# Test 8: Check for potential conflicts
log_test "Checking for potential conflicts..."
if [ -d "$HOME/.config/zsh" ] && [ ! -L "$HOME/.config/zsh" ]; then
    log_warn "~/.config/zsh exists and is not a symlink (may conflict with stow)"
fi
if [ -d "$HOME/.config/fish" ] && [ ! -L "$HOME/.config/fish" ]; then
    log_warn "~/.config/fish exists and is not a symlink (may conflict with stow)"
fi
if [ -d "$HOME/.config/nushell" ] && [ ! -L "$HOME/.config/nushell" ]; then
    log_warn "~/.config/nushell exists and is not a symlink (may conflict with stow)"
fi
echo ""

# Test 9: Dry-run test for Cargo packages
log_test "Testing Cargo package availability (sample)..."
if command -v cargo &>/dev/null; then
    SAMPLE_PACKAGES=("cargo-watch" "cargo-make" "just")
    for pkg in "${SAMPLE_PACKAGES[@]}"; do
        if cargo search "$pkg" --limit 1 &>/dev/null; then
            log_info "$pkg is available on crates.io"
        else
            log_warn "$pkg search failed"
        fi
    done
else
    log_warn "Cargo not installed, skipping package availability check"
fi
echo ""

echo "=== Test Complete ==="
echo ""
echo "To safely test the installation:"
echo "  1. Use Docker: docker run -it --rm -v \$(pwd):/dotconfig ubuntu:latest"
echo "  2. Use test-docker.sh in this repository"
echo "  3. Create a backup: tar -czf ~/dotconfig-backup.tar.gz ~/.config"
echo ""
echo "To proceed with installation:"
echo "  ./install.sh"
