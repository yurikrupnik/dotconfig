# Quick Start Guide

## Installation Methods

### Method 1: One-liner (Recommended for fresh machines)

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/yourusername/dotconfig/main/bootstrap.sh)
```

### Method 2: Clone and install

```bash
git clone https://github.com/yourusername/dotconfig.git ~/dotconfig
cd ~/dotconfig
./install.sh
```

### Method 3: Using Make

```bash
git clone https://github.com/yourusername/dotconfig.git ~/dotconfig
cd ~/dotconfig
make install
```

## Daily Usage

### Update everything
```bash
cd ~/dotconfig
make update
# or
./update.sh
```

### Modify shell configurations
```bash
# 1. Edit the source file
vim ~/dotconfig/scripts/nu/setup-local-machine/config.toml

# 2. Regenerate and apply
cd ~/dotconfig
make regen
```

### Install new Homebrew package
```bash
# Add to ~/dotconfig/brew/Brewfile
echo 'brew "neovim"' >> ~/dotconfig/brew/Brewfile

# Install
make brew-install
```

### Install new Cargo tool
```bash
# Add to ~/dotconfig/cargo-install.toml
echo 'cargo-watch' >> ~/dotconfig/cargo-install.toml

# Install (uses cargo-binstall for faster installation)
make cargo-install

# Or install manually with binstall:
cargo binstall cargo-watch --no-confirm
```

**Note**: cargo-binstall downloads pre-compiled binaries when available, making installations 10-100x faster than compiling from source.

### Install new npm/bun package
```bash
# Add to ~/dotconfig/global-packages.json
# Then run
make npm-install
```

## Common Commands

```bash
make install        # Bootstrap entire system
make update         # Update everything
make generate       # Generate shell configs
make stow           # Apply configs
make unstow         # Remove configs
make regen          # Generate + stow
make build          # Build dotconfig CLI
make test           # Run tests
make help           # Show all commands
```

## What Gets Installed?

- **Homebrew**: Package manager for macOS/Linux
- **100+ packages**: Development tools, CLI utilities, apps
- **Rust toolchain**: Latest stable Rust + components
- **cargo-binstall**: Fast binary installer for Cargo packages
- **Global Cargo tools**: cargo-watch, bacon, etc. (installed via pre-compiled binaries)
- **Global npm/bun packages**: TypeScript, tsx, etc.
- **Shell configs**: Generated for zsh, fish, nushell
- **dotconfig CLI**: Custom Rust tool for local dev

## File Structure

```
~/dotconfig/
├── install.sh          # Fresh install
├── update.sh           # Update all
├── bootstrap.sh        # Remote bootstrap
├── Makefile            # Convenience commands
├── brew/Brewfile       # System packages
├── cargo-install.toml  # Rust tools
├── global-packages.json# Node tools
└── scripts/nu/setup-local-machine/
    ├── config.toml     # Shell config source
    └── shells.nu       # Generator script
```

## After Installation

1. Restart your shell: `exec $SHELL`
2. Test the installation: `dotconfig --help`
3. Check shell config: `which nu` (or zsh/fish)
4. Update when needed: `make update`

## Troubleshooting

### Command not found after install
```bash
# Add to PATH manually
export PATH="$HOME/dotconfig/target/release:$PATH"
```

### Homebrew not in PATH
```bash
# macOS
eval "$(/opt/homebrew/bin/brew shellenv)"

# Linux
eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
```

### Configurations not loading
```bash
# Check if stowed correctly
ls -la ~/.config/nushell
ls -la ~/.config/zsh

# Re-apply
make regen
```

## Need Help?

- Full docs: [INSTALLATION.md](./INSTALLATION.md)
- Project structure: [README.md](./README.md)
- Issues: GitHub Issues
