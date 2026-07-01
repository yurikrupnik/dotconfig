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
for cmd in just cargo-binstall cargo-liner bun uv; do
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
    "$DOTCONFIG_DIR/config/node/package.json" \
    "$DOTCONFIG_DIR/config/uv/tools.txt" \
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
        warn "$path not present (package may be unstowed)"
        return
    fi
    if [ -L "$path" ]; then
        # Tree-folded: whole directory or file is a single symlink.
        local resolved
        resolved="$(readlink "$path")"
        case "$resolved" in
            *dotconfig/*) ok "$path → ${resolved##*dotconfig/}" ;;
            *) bad "$path → $resolved (not from dotconfig)" ;;
        esac
        return
    fi
    if [ -d "$path" ]; then
        # Unfolded: real directory whose contents are symlinks into dotconfig.
        local stowed
        stowed="$(find "$path" -maxdepth 1 -type l -lname '*dotconfig*' | head -1)"
        if [ -n "$stowed" ]; then
            ok "$path/ (unfolded; e.g. ${stowed##$path/} → $(readlink "$stowed"))"
        else
            bad "$path is a directory but contains no symlinks into dotconfig"
        fi
        return
    fi
    bad "$path is a regular file (stow conflict?)"
}

check_stow "$HOME/.zshenv"
check_stow "$HOME/.config/zsh"
check_stow "$HOME/.config/nushell"
check_stow "$HOME/.config/starship"
check_stow "$HOME/.config/zed"
check_stow "$HOME/.local/bin/up"
check_stow "$HOME/.local/bin/csort"

# 5. Generated output freshness
section "Generated output freshness"
CONFIG_TOML="$DOTCONFIG_DIR/config/shell/config.toml"
check_freshness() {
    local label="$1"
    local gen_file="$2"
    if [ -f "$CONFIG_TOML" ] && [ -f "$gen_file" ]; then
        if [ "$CONFIG_TOML" -nt "$gen_file" ]; then
            warn "$label is stale (config.toml newer) — run 'just regen'"
        else
            ok "$label is up-to-date with config.toml"
        fi
    elif [ ! -f "$gen_file" ]; then
        bad "$label has not been generated — run 'just generate'"
    fi
}
check_freshness "output/zsh" "$DOTCONFIG_DIR/output/zsh/.config/zsh/generated.zsh"
check_freshness "output/nu"  "$DOTCONFIG_DIR/output/nu/.config/nushell/generated.nu"

# 6. No hardcoded /Users/<username> in tracked files (output/ is gitignored,
#    so generated absolute paths don't trip this check).
section "No hardcoded user-specific paths"
if command -v git >/dev/null 2>&1 && [ -d "$DOTCONFIG_DIR/.git" ]; then
    hits=$(cd "$DOTCONFIG_DIR" && git ls-files -z | xargs -0 grep -lE "/Users/[a-zA-Z][a-zA-Z0-9_-]+" 2>/dev/null || true)
    if [ -n "$hits" ]; then
        bad "Hardcoded /Users/<username> in tracked files:"
        echo "$hits" | sed 's/^/    /'
    else
        ok "No hardcoded /Users/<username> in tracked files"
    fi
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
