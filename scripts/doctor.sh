#!/usr/bin/env bash
# Health check for the dotconfig setup.
# Exits 0 if everything looks good, 1 if any check fails.

set -uo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
DIM='\033[2m'
NC='\033[0m'

DOTCONFIG_DIR="${DOTCONFIG_DIR:-$HOME/dotconfig}"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

PASS=0
FAIL=0
WARN=0

ok()   { echo -e "  ${GREEN}✓${NC} $1"; PASS=$((PASS+1)); }
bad()  { echo -e "  ${RED}✗${NC} $1"; FAIL=$((FAIL+1)); }
warn() { echo -e "  ${YELLOW}!${NC} $1"; WARN=$((WARN+1)); }
section() { echo -e "\n${DIM}── $1 ──${NC}"; }

# 1. Required commands
section "Required commands"
for cmd in brew cargo rustup nu stow git; do
    if command -v "$cmd" &> /dev/null; then
        ok "$cmd ($(command -v "$cmd"))"
    else
        bad "$cmd not found in PATH"
    fi
done

# Optional but recommended
for cmd in just cargo-binstall cargo-liner bun; do
    if command -v "$cmd" &> /dev/null; then
        ok "$cmd"
    else
        warn "$cmd not installed (optional)"
    fi
done

# 2. Source-of-truth files
section "Config manifests"
for f in \
    "$DOTCONFIG_DIR/config/brew/Brewfile" \
    "$DOTCONFIG_DIR/config/cargo/liner.toml" \
    "$DOTCONFIG_DIR/config/node/global-packages.json" \
    "$DOTCONFIG_DIR/config/shell/config.toml"; do
    if [ -f "$f" ]; then
        ok "${f#$DOTCONFIG_DIR/}"
    else
        bad "missing: ${f#$DOTCONFIG_DIR/}"
    fi
done

# 3. cargo-liner symlink
section "cargo-liner config link"
LINER_SRC="$DOTCONFIG_DIR/config/cargo/liner.toml"
LINER_DEST="$CARGO_HOME_DIR/liner.toml"
if [ -L "$LINER_DEST" ]; then
    target="$(readlink "$LINER_DEST")"
    if [ "$target" = "$LINER_SRC" ]; then
        ok "$LINER_DEST → $LINER_SRC"
    else
        bad "$LINER_DEST points to $target (expected $LINER_SRC)"
    fi
elif [ -e "$LINER_DEST" ]; then
    bad "$LINER_DEST exists but is not a symlink"
else
    warn "$LINER_DEST not linked (run ./install.sh)"
fi

# 4. Stowed symlinks
section "Stowed symlinks resolve into dotconfig"
check_stow() {
    local path="$1"
    if [ ! -e "$path" ] && [ ! -L "$path" ]; then
        warn "$path not present (shell may be unstowed)"
        return
    fi
    if [ ! -L "$path" ]; then
        bad "$path exists but is not a symlink (stow conflict?)"
        return
    fi
    local resolved
    resolved="$(readlink "$path")"
    case "$resolved" in
        *dotconfig/output/*) ok "$path → ${resolved##*dotconfig/}" ;;
        *) bad "$path → $resolved (not from dotconfig/output)" ;;
    esac
}

check_stow "$HOME/.zshenv"
check_stow "$HOME/.config/zsh"
check_stow "$HOME/.config/fish"
check_stow "$HOME/.config/nushell"
check_stow "$HOME/.config/bash"
check_stow "$HOME/.config/starship"

# 5. Generated output freshness
section "Generated output freshness"
CONFIG_TOML="$DOTCONFIG_DIR/config/shell/config.toml"
ZSH_GEN="$DOTCONFIG_DIR/output/zsh/.config/zsh/generated.zsh"
if [ -f "$CONFIG_TOML" ] && [ -f "$ZSH_GEN" ]; then
    if [ "$CONFIG_TOML" -nt "$ZSH_GEN" ]; then
        warn "config.toml is newer than output/ — run 'just regen'"
    else
        ok "output/ is up-to-date with config.toml"
    fi
elif [ ! -f "$ZSH_GEN" ]; then
    bad "output/ has not been generated — run 'just generate'"
fi

# Summary
echo ""
if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}All checks passed.${NC} ${PASS} ok, ${WARN} warnings."
    exit 0
else
    echo -e "${RED}${FAIL} check(s) failed.${NC} ${PASS} ok, ${WARN} warnings."
    exit 1
fi
