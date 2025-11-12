# Dotconfig Installation Guide

A single-command dotfiles generator and development environment setup.

## Quick Start

### Fresh Machine Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/dotconfig.git ~/dotconfig

# Run the installation
cd ~/dotconfig
./install.sh
```

That's it! The script will:
1. Install Homebrew (if not present)
2. Install all packages from Brewfile
3. Set up Rust toolchain
4. Install cargo-binstall (for faster Cargo package installations)
5. Install global Cargo packages (using pre-compiled binaries when available)
6. Install global npm/bun packages
7. Generate shell configurations (zsh, fish, nushell)
8. Stow configurations to your home directory
9. Build the `dotconfig` CLI tool

### Updating Your System

```bash
cd ~/dotconfig
./update.sh
```

## Package Management

### Homebrew Packages

Edit `brew/Brewfile` and run:
```bash
brew bundle --file=~/dotconfig/brew/Brewfile
```

### Global Cargo Packages

Edit `cargo-install.toml` (one package per line):
```toml
cargo-watch
cargo-nextest
bacon
```

Then install:
```bash
# Using cargo-binstall (faster - installs pre-compiled binaries)
cargo install cargo-binstall
cat cargo-install.toml | grep '^[a-z]' | while read pkg; do
    cargo binstall "$pkg" --no-confirm
done
```

### Global npm/bun Packages

Edit `global-packages.json` dependencies:
```json
{
  "dependencies": {
    "typescript": "latest",
    "tsx": "latest"
  }
}
```

Then install:
```bash
# Using bun (faster)
bun install --global

# Or using npm
npm install --global
```

## Shell Configuration

### Single Source of Truth

All shell configurations are generated from `scripts/nu/setup-local-machine/config.toml`:

```toml
[aliases]
lg = "lazygit"
k = "kubectl"

[functions]
update = """
brew bundle --file ~/dotconfig/brew/Brewfile
rustup update
nu shells.nu generate
"""

[env]
EDITOR = "zed"
```

### Generate Configurations

```bash
cd ~/dotconfig/scripts/nu/setup-local-machine

# Generate shell-specific configs
nu shells.nu generate

# Apply to home directory
nu shells.nu stow

# Remove from home directory
nu shells.nu unstow
```

This generates:
- `output/zsh/.config/zsh/generated.zsh`
- `output/fish/.config/fish/generated_*.fish`
- `output/nu/.config/nushell/generated.nu`

## Architecture

```
dotconfig/
├── install.sh              # Bootstrap script
├── update.sh               # Update all tools
├── brew/Brewfile           # Homebrew packages
├── cargo-install.toml      # Global Cargo packages
├── global-packages.json    # Global npm/bun packages
├── scripts/
│   └── nu/
│       └── setup-local-machine/
│           ├── config.toml # Shell configuration source
│           └── shells.nu   # Configuration generator
├── output/                 # Generated configurations
│   ├── zsh/
│   ├── fish/
│   └── nu/
└── src/                    # Rust CLI tool

```

## Supported Platforms

- macOS (Intel and Apple Silicon)
- Linux (via Homebrew on Linux)

## Requirements

- Bash (for bootstrap)
- Internet connection
- Git

Everything else is installed by the script.

## Customization

1. **Add aliases/functions**: Edit `scripts/nu/setup-local-machine/config.toml`
2. **Add system packages**: Edit `brew/Brewfile`
3. **Add Rust tools**: Edit `cargo-install.toml`
4. **Add Node.js tools**: Edit `global-packages.json`
5. **Regenerate**: Run `nu shells.nu generate && nu shells.nu stow`

## Troubleshooting

### Homebrew not found after install
```bash
# macOS
eval "$(/opt/homebrew/bin/brew shellenv)"

# Linux
eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
```

### Shell configurations not loaded
```bash
# Restart your shell
exec $SHELL

# Or source manually
source ~/.zshrc  # or ~/.config/fish/config.fish or ~/.config/nushell/config.nu
```

### Cargo install fails
```bash
# The install script uses cargo-binstall for faster installs (pre-compiled binaries)
# If a package doesn't have a binary available, it will compile from source
# Check if cargo-binstall is working:
cargo-binstall --version

# Manually install a package:
cargo binstall <package-name> --no-confirm

# Fallback to compiling from source:
cargo install <package-name>
```

## Advanced Usage

### Manual Steps

If you want to run individual steps:

```bash
# 1. Install Homebrew
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 2. Install packages
brew bundle --file=~/dotconfig/brew/Brewfile

# 3. Generate shells
cd ~/dotconfig/scripts/nu/setup-local-machine
nu shells.nu generate
nu shells.nu stow

# 4. Install global cargo packages
cargo install cargo-binstall
cat ~/dotconfig/cargo-install.toml | grep '^[a-z]' | while read pkg; do
    cargo binstall "$pkg" --no-confirm
done

# 5. Install global npm/bun packages
cd ~/dotconfig
bun install --global  # or npm install --global

# 6. Build CLI
cd ~/dotconfig
cargo build --release
```

### CI/CD Integration

You can use this in CI environments:

```yaml
# GitHub Actions example
- name: Setup development environment
  run: |
    git clone https://github.com/yourusername/dotconfig.git ~/dotconfig
    cd ~/dotconfig
    ./install.sh
```

## Philosophy

- **Single source of truth**: `config.toml` for all shell configurations
- **Cross-shell compatibility**: Generate shell-specific syntax from one config
- **Declarative package management**: All tools defined in version-controlled files
- **Reproducible environments**: Same setup on every machine
- **Fast updates**: Incremental updates via `update.sh`
