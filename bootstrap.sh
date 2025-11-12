#!/usr/bin/env bash
# Bootstrap script for dotconfig - can be run via curl
set -e

REPO_URL="${DOTCONFIG_REPO:-https://github.com/yourusername/dotconfig.git}"
INSTALL_DIR="${DOTCONFIG_DIR:-$HOME/dotconfig}"

echo "Bootstrapping dotconfig..."
echo "Repository: $REPO_URL"
echo "Install directory: $INSTALL_DIR"
echo ""

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
    git pull
else
    echo "Cloning repository..."
    git clone "$REPO_URL" "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

# Run installation
echo ""
echo "Running installation..."
./install.sh
