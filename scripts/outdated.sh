#!/usr/bin/env bash
# Preview what `u` would refresh — read-only.
set -uo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
DIM='\033[2m'
NC='\033[0m'

section() { echo -e "\n${GREEN}==>${NC} $1"; }
dim()     { echo -e "${DIM}$1${NC}"; }

if command -v brew &> /dev/null; then
    section "Homebrew"
    out=$(brew outdated --verbose 2>/dev/null || true)
    if [ -z "$out" ]; then
        dim "  all packages up to date"
    else
        echo "$out" | sed 's/^/  /'
    fi
fi

if command -v rustup &> /dev/null; then
    section "Rust toolchain"
    rustup check 2>/dev/null | sed 's/^/  /'
fi

if command -v bun &> /dev/null; then
    section "Bun globals"
    out=$(bun outdated --global 2>/dev/null || true)
    if [ -z "$out" ]; then
        dim "  all globals up to date"
    else
        echo "$out" | sed 's/^/  /'
    fi
elif command -v npm &> /dev/null; then
    section "Npm globals"
    out=$(npm outdated --global 2>/dev/null || true)
    if [ -z "$out" ]; then
        dim "  all globals up to date"
    else
        echo "$out" | sed 's/^/  /'
    fi
fi

if command -v uv &> /dev/null; then
    section "uv tools"
    out=$(uv tool list --outdated 2>/dev/null || true)
    if [ -z "$out" ]; then
        dim "  all tools up to date"
    else
        echo "$out" | sed 's/^/  /'
    fi
fi

echo -e "\n${YELLOW}==>${NC} Run ${GREEN}u${NC} (shell function) to apply."
