#!/usr/bin/env bash
# Bootstrap script for dotconfig - can be run via curl
set -euo pipefail

REPO_URL="${DOTCONFIG_REPO:-https://github.com/yurikrupnik/dotconfig.git}"
INSTALL_DIR="${DOTCONFIG_DIR:-$HOME/dotconfig}"

echo "Bootstrapping dotconfig..."
echo "Repository: $REPO_URL"
echo "Install directory: $INSTALL_DIR"
echo ""

# macOS: /usr/bin/git is a stub until the Command Line Tools are installed, so
# `command -v git` succeeds even when git cannot run. Check the real toolchain.
if [ "$(uname -s)" = "Darwin" ]; then
    if ! xcode-select -p &> /dev/null; then
        echo "Error: Xcode Command Line Tools are not installed (git, clang, make)."
        echo "Opening the installer: xcode-select --install"
        xcode-select --install || true
        echo ""
        echo "Finish the Command Line Tools install dialog, then re-run this bootstrap."
        exit 1
    fi
fi

# Check if git is installed
if ! command -v git &> /dev/null; then
    echo "Error: git is not installed. Please install git first."
    echo ""
    echo "macOS: xcode-select --install"
    echo "Linux (Debian/Ubuntu): sudo apt-get install git"
    echo "Linux (RHEL/Fedora): sudo dnf install git"
    exit 1
fi

# Clone or update repository
if [ -d "$INSTALL_DIR" ]; then
    echo "Directory $INSTALL_DIR already exists. Updating..."
    cd "$INSTALL_DIR"
    # --ff-only: never create a merge commit or rewrite local work; if the clone
    # has diverged, stop and let the user resolve it.
    if ! git pull --ff-only; then
        echo "Error: 'git pull --ff-only' failed in $INSTALL_DIR (local changes or diverged history)."
        echo "Resolve it manually, then re-run this bootstrap."
        exit 1
    fi
else
    echo "Cloning repository..."
    git clone "$REPO_URL" "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

# Run installation
echo ""
echo "Running installation..."
./install.sh
