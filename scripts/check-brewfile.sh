#!/usr/bin/env bash
# Verify every `tap`, `brew` and `cask` entry in a Brewfile resolves to a real
# tap / formula / cask — without installing anything.
#
# Usage: scripts/check-brewfile.sh [--tap] [BREWFILE]
#
#   BREWFILE  defaults to config/brew/Brewfile in this repo.
#   --tap     `brew tap` any declared tap (explicit `tap "o/t"` or implied by
#             `brew "o/t/name"`) that is not tapped yet. Without it, missing
#             taps are reported as failures and their entries are skipped
#             (so nothing on the machine is modified).
#
# Collects all failures, prints them, exits 1 if any.
set -uo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
DIM='\033[2m'
NC='\033[0m'

log_info() { echo -e "${GREEN}==>${NC} $1"; }
log_warn() { echo -e "${YELLOW}!${NC} $1" >&2; }
log_err()  { echo -e "${RED}✗${NC} $1" >&2; }

usage() { sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; }

DO_TAP=0
BREWFILE=""
for arg in "$@"; do
    case "$arg" in
        --tap)      DO_TAP=1 ;;
        -h|--help)  usage; exit 0 ;;
        -*)         log_err "Unknown arg: $arg"; usage >&2; exit 2 ;;
        *)
            if [ -n "$BREWFILE" ]; then
                log_err "Only one Brewfile path allowed (got '$BREWFILE' and '$arg')"
                exit 2
            fi
            BREWFILE="$arg"
            ;;
    esac
done

DOTCONFIG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BREWFILE="${BREWFILE:-$DOTCONFIG_DIR/config/brew/Brewfile}"

if [ ! -f "$BREWFILE" ]; then
    log_err "Brewfile not found: $BREWFILE"
    exit 1
fi
if ! command -v brew >/dev/null 2>&1; then
    log_err "brew is not on PATH"
    exit 1
fi

# Read-only checks must not trigger a (slow, mutating) `brew update`.
export HOMEBREW_NO_AUTO_UPDATE="${HOMEBREW_NO_AUTO_UPDATE:-1}"
export HOMEBREW_NO_ENV_HINTS=1

# `<kind> <name>` per declared entry (comments and other kinds ignored).
ENTRIES="$(sed -nE 's/^[[:space:]]*(tap|brew|cask)[[:space:]]+"([^"]+)".*/\1 \2/p' "$BREWFILE")"
if [ -z "$ENTRIES" ]; then
    log_err "No tap/brew/cask entries found in $BREWFILE"
    exit 1
fi

lower() { printf '%s\n' "$1" | tr '[:upper:]' '[:lower:]'; }

# Tap of a qualified name (`owner/tap/name` -> `owner/tap`), empty otherwise.
tap_of() {
    case "$1" in
        */*/*) lower "${1%/*}" ;;
        *)     echo "" ;;
    esac
}

FAILURES=""
N_FAIL=0
fail() {
    FAILURES="${FAILURES}  - $1"$'\n'
    N_FAIL=$((N_FAIL + 1))
}

# ── Taps ─────────────────────────────────────────────────────────────────────
DECLARED_TAPS="$(
    while read -r kind name; do
        if [ "$kind" = "tap" ]; then lower "$name"; else tap_of "$name"; fi
    done <<< "$ENTRIES" | sed '/^$/d' | sort -u
)"
TAPPED="$(brew tap 2>/dev/null | tr '[:upper:]' '[:lower:]' | sort -u)"

is_tapped() { printf '%s\n' "$TAPPED" | grep -qxF "$1"; }

MISSING_TAPS=""
N_TAPS=0
while read -r t; do
    [ -z "$t" ] && continue
    N_TAPS=$((N_TAPS + 1))
    is_tapped "$t" && continue
    if [ "$DO_TAP" -eq 1 ]; then
        log_info "brew tap $t"
        if out="$(brew tap "$t" 2>&1 </dev/null)"; then
            TAPPED="$(printf '%s\n%s\n' "$TAPPED" "$t")"
            continue
        fi
        fail "tap \"$t\": $(printf '%s\n' "$out" | grep -m1 -iE 'error|fatal' || printf '%s\n' "$out" | tail -n1)"
    else
        fail "tap \"$t\": not tapped (rerun with --tap)"
    fi
    MISSING_TAPS="${MISSING_TAPS}${t}"$'\n'
done <<< "$DECLARED_TAPS"

tap_missing() { [ -n "$1" ] && printf '%s' "$MISSING_TAPS" | grep -qxF "$1"; }

# ── Formulae / casks ─────────────────────────────────────────────────────────
# Names whose tap is unavailable are skipped: querying them would make brew
# auto-tap, which this script must only do under --tap.
names_for() {
    local want="$1" kind name
    while read -r kind name; do
        [ "$kind" = "$want" ] || continue
        tap_missing "$(tap_of "$name")" && continue
        printf '%s\n' "$name"
    done <<< "$ENTRIES"
}

# Batch query first (one brew startup); on failure, query one-by-one to name
# every offender — brew aborts the batch at the first unknown name.
check_kind() {
    local kind="$1" flag="$2" names n=0 name out
    names="$(names_for "$kind")"
    [ -z "$names" ] && { echo 0; return; }
    n="$(printf '%s\n' "$names" | wc -l | tr -d ' ')"
    # shellcheck disable=SC2086  # word-splitting the name list is intended
    if brew info --json=v2 "$flag" $names >/dev/null 2>&1; then
        echo "$n"
        return
    fi
    while IFS= read -r name; do
        if ! out="$(brew info --json=v2 "$flag" "$name" 2>&1 >/dev/null </dev/null)"; then
            fail "$kind \"$name\": $(printf '%s\n' "$out" | grep -m1 -i 'error' || printf '%s\n' "$out" | tail -n1)"
        fi
    done <<< "$names"
    echo "$n"
}

# check_kind runs in the current shell (not a subshell) so fail() persists;
# its count goes to a temp file instead of command substitution.
COUNT_FILE="$(mktemp)"
trap 'rm -f "$COUNT_FILE"' EXIT
check_kind brew --formula > "$COUNT_FILE"; N_FORMULAE="$(cat "$COUNT_FILE")"
check_kind cask --cask    > "$COUNT_FILE"; N_CASKS="$(cat "$COUNT_FILE")"

log_info "brewfile check ${DIM}($BREWFILE)${NC}"
printf "    %-9s %3d checked\n" "Taps" "$N_TAPS" "Formulae" "$N_FORMULAE" "Casks" "$N_CASKS"

if [ "$N_FAIL" -gt 0 ]; then
    log_err "$N_FAIL problem(s):"
    printf '%s' "$FAILURES" >&2
    if [ -n "${HOMEBREW_REQUIRE_TAP_TRUST:-}" ]; then
        log_warn "HOMEBREW_REQUIRE_TAP_TRUST is set — untrusted tap entries fail to load; see scripts/brew-preflight.sh"
    fi
    exit 1
fi
echo -e "${GREEN}✓${NC} all entries resolve"
