#!/usr/bin/env bash
# Pre-flight for `brew bundle` against config/brew/Brewfile.
#
# --check   (default) Read-only report: count formulae/casks/taps declared
#           in the Brewfile and list which third-party taps are not yet
#           trusted by Homebrew (HOMEBREW_REQUIRE_TAP_TRUST).
# --apply   Same report, then trust each pending tap (after a Y/n prompt,
#           or unconditionally if BREW_TRUST_NEW_TAPS=1) and run
#           `brew bundle`. Refuses to trust silently when stdin/stdout
#           are not a TTY.
set -uo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
DIM='\033[2m'
NC='\033[0m'

MODE="check"
case "${1:-}" in
    --apply)         MODE="apply" ;;
    --check|"")      MODE="check" ;;
    -h|--help)
        sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *) echo "Unknown arg: $1" >&2; exit 2 ;;
esac

DOTCONFIG_DIR="${DOTCONFIG_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
BREWFILE="${BREWFILE:-$DOTCONFIG_DIR/config/brew/Brewfile}"

if [ ! -f "$BREWFILE" ]; then
    echo -e "${RED}✗${NC} Brewfile not found: $BREWFILE" >&2
    exit 1
fi
if ! command -v brew >/dev/null 2>&1; then
    echo -e "${RED}✗${NC} brew is not on PATH" >&2
    exit 1
fi

# Declared taps: explicit `tap "owner/name"` plus implicit owner/name from
# `brew "owner/name/formula"` and `cask "owner/name/formula"`.
mapfile -t DECLARED_TAPS < <(awk '
    /^[[:space:]]*tap "/ {
        match($0, /"[^"]+"/); print tolower(substr($0, RSTART+1, RLENGTH-2)); next
    }
    /^[[:space:]]*(brew|cask) "[^"]*\/[^"]*\/[^"]*"/ {
        match($0, /"[^"]+"/); s = substr($0, RSTART+1, RLENGTH-2);
        n = split(s, p, "/"); if (n >= 3) print tolower(p[1]"/"p[2]);
    }
' "$BREWFILE" | sort -u)

N_FORMULAE=$(grep -cE '^[[:space:]]*brew "' "$BREWFILE" || true)
N_CASKS=$(grep -cE   '^[[:space:]]*cask "' "$BREWFILE" || true)

# Trusted taps (parse `brew trust --json v1`; python3 is on every modern macOS,
# avoids a hard dependency on jq during the fresh-machine bootstrap).
mapfile -t TRUSTED_TAPS < <(brew trust --json v1 2>/dev/null | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
    for t in data.get("taps", []) or []:
        print(t.lower())
except Exception:
    pass
' 2>/dev/null | sort -u)

# Diff: declared taps that are not yet trusted.
PENDING=()
for tap in "${DECLARED_TAPS[@]+"${DECLARED_TAPS[@]}"}"; do
    found=0
    for t in "${TRUSTED_TAPS[@]+"${TRUSTED_TAPS[@]}"}"; do
        [ "$tap" = "$t" ] && { found=1; break; }
    done
    [ "$found" -eq 0 ] && PENDING+=("$tap")
done

# Report
echo -e "${GREEN}==>${NC} brew preflight ${DIM}($BREWFILE)${NC}"
printf "    %-9s %3d declared, %3d trusted, %3d pending trust\n" \
    "Taps"     "${#DECLARED_TAPS[@]}" "${#TRUSTED_TAPS[@]}" "${#PENDING[@]}"
printf "    %-9s %3d declared\n" "Formulae" "$N_FORMULAE"
printf "    %-9s %3d declared\n" "Casks"    "$N_CASKS"

if [ "${#PENDING[@]}" -gt 0 ]; then
    echo -e "\n${YELLOW}    Pending trust:${NC}"
    for t in "${PENDING[@]}"; do echo "      - $t"; done
fi

if [ "$MODE" = "check" ]; then
    exit 0
fi

# --apply: trust pending taps (with confirmation) then run `brew bundle`.
if [ "${#PENDING[@]}" -gt 0 ]; then
    if [ "${BREW_TRUST_NEW_TAPS:-0}" = "1" ]; then
        echo -e "\n${GREEN}==>${NC} BREW_TRUST_NEW_TAPS=1 — trusting ${#PENDING[@]} tap(s)"
    elif [ -t 0 ] && [ -t 1 ]; then
        printf "\n${YELLOW}==>${NC} Trust these %d tap(s)? [y/N] " "${#PENDING[@]}"
        read -r REPLY
        case "${REPLY:-}" in
            [yY]|[yY][eE][sS]) ;;
            *) echo "Aborted. Set BREW_TRUST_NEW_TAPS=1 to skip the prompt."; exit 1 ;;
        esac
    else
        echo -e "${RED}✗${NC} Refusing to trust ${#PENDING[@]} new tap(s) non-interactively." >&2
        echo   "  Set BREW_TRUST_NEW_TAPS=1 to override." >&2
        exit 1
    fi
    for t in "${PENDING[@]}"; do
        brew trust --tap "$t"
    done
fi

echo -e "\n${GREEN}==>${NC} brew bundle"
exec brew bundle --file="$BREWFILE"
