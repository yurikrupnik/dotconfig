#!/bin/bash

# .config Setup Script
# This script helps set up the configuration files in your system

set -e

echo "🚀 Setting up .config..."

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")"

echo "📁 Configuration directory: $CONFIG_DIR"

# Function to create symlinks safely
create_symlink() {
    local source="$1"
    local target="$2"
    
    if [ -e "$target" ] && [ ! -L "$target" ]; then
        echo "⚠️  Backing up existing file: $target"
        mv "$target" "$target.backup"
    fi
    
    ln -sf "$source" "$target"
    echo "✅ Linked: $target -> $source"
}

# Example: Link git configuration (uncomment when you add git configs)
# create_symlink "$CONFIG_DIR/git/.gitconfig" "$HOME/.gitconfig"

echo "✨ Setup complete!"
echo "💡 Add your configuration files to the appropriate directories and update this script to create symlinks."