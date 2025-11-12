#!/usr/bin/env bash
# Backup existing configurations before installation
set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}==>${NC} $1"
}

BACKUP_DIR="$HOME/dotconfig-backup-$(date +%Y%m%d-%H%M%S)"

log_info "Creating backup at: $BACKUP_DIR"
mkdir -p "$BACKUP_DIR"

# Backup shell configurations
CONFIGS=(
    ".zshrc"
    ".bashrc"
    ".config/zsh"
    ".config/fish"
    ".config/nushell"
    ".config/starship.toml"
)

for config in "${CONFIGS[@]}"; do
    if [ -e "$HOME/$config" ]; then
        log_info "Backing up ~/$config"
        mkdir -p "$BACKUP_DIR/$(dirname "$config")"
        cp -r "$HOME/$config" "$BACKUP_DIR/$config"
    fi
done

# Create restore script
cat > "$BACKUP_DIR/restore.sh" << 'EOF'
#!/usr/bin/env bash
# Restore backup
set -e

BACKUP_DIR="$(cd "$(dirname "$0")" && pwd)"
echo "Restoring from: $BACKUP_DIR"

# Remove stowed configs
if [ -d "$HOME/dotconfig" ]; then
    cd "$HOME/dotconfig/scripts/nu/setup-local-machine"
    nu shells.nu unstow 2>/dev/null || true
fi

# Restore files
cd "$BACKUP_DIR"
for item in .* *; do
    if [ "$item" != "." ] && [ "$item" != ".." ] && [ "$item" != "restore.sh" ]; then
        echo "Restoring ~/$item"
        cp -r "$item" "$HOME/"
    fi
done

echo "Restore complete!"
EOF

chmod +x "$BACKUP_DIR/restore.sh"

log_info "Backup complete!"
echo ""
echo "Backup location: $BACKUP_DIR"
echo "To restore: $BACKUP_DIR/restore.sh"
echo ""
