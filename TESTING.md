# Testing Guide

Safe ways to test the dotconfig installation scripts without breaking your system.

## Quick Safety Check

Before any installation, run the validation script:

```bash
./test-install.sh
```

This will:
- Check all required files exist
- Validate script permissions
- Validate Brewfile, cargo-install.toml, and global-packages.json syntax
- Show what would be installed
- Check for potential conflicts
- No changes to your system

## Testing Methods

### Method 1: Docker Testing (Recommended)

**Safest option** - Test in complete isolation.

```bash
# Interactive test runner
./test-docker.sh

# Or manually:
docker build -f Dockerfile.test -t dotconfig-test .
docker run -it --rm dotconfig-test ./test-install.sh   # Validation only
docker run -it --rm dotconfig-test ./install.sh        # Full install
```

**Pros:**
- Complete isolation from your system
- Can test multiple times
- No risk to existing configuration

**Cons:**
- Requires Docker
- Takes longer due to container setup

### Method 2: Create a Backup First

Create a backup before testing on your actual system:

```bash
# Create backup
./backup.sh

# This creates a timestamped backup at:
# ~/dotconfig-backup-YYYYMMDD-HHMMSS/

# Test installation
./install.sh

# If something goes wrong, restore:
~/dotconfig-backup-YYYYMMDD-HHMMSS/restore.sh
```

### Method 3: Test Individual Components

Test each component separately without running the full install:

#### Test Homebrew packages
```bash
# Check Brewfile syntax
brew bundle check --file=brew/Brewfile

# See what would be installed
brew bundle list --file=brew/Brewfile

# Install in dry-run mode (check what's missing)
brew bundle --file=brew/Brewfile --no-lock
```

#### Test Cargo packages
```bash
# Validate cargo-install.toml
cat cargo-install.toml | grep '^[a-z]'

# Test installing one package
cargo binstall --dry-run cargo-watch --no-confirm

# Or test with cargo-binstall's simulation
cargo binstall --help
```

#### Test shell configuration generation
```bash
cd scripts/nu/setup-local-machine

# Generate configs (doesn't apply them)
nu shells.nu generate

# Check generated files
ls -la ../../output/

# View generated config
cat ../../output/zsh/.config/zsh/generated.zsh

# Test stow without actually applying (shows what would happen)
stow --simulate -t ~ -d ../../output zsh
```

#### Test Nushell scripts
```bash
# Validate Nu syntax
nu -c "use scripts/nu/setup-local-machine/shells.nu"

# Check config.toml
cat scripts/nu/setup-local-machine/config.toml
```

### Method 4: Virtual Machine Testing

Test on a fresh VM (if you have access):

```bash
# On the VM
git clone <your-repo> ~/dotconfig
cd ~/dotconfig
./install.sh
```

**Pros:**
- Real system test
- Can snapshot/restore VM state

### Method 5: Incremental Testing on Your System

If you want to test on your actual system, do it incrementally:

```bash
# 1. Backup first
./backup.sh

# 2. Test validation
./test-install.sh

# 3. Install Homebrew packages only
brew bundle --file=brew/Brewfile

# 4. Install cargo-binstall
cargo install cargo-binstall

# 5. Install one Cargo package
cargo binstall cargo-watch --no-confirm

# 6. Generate shell configs (don't stow yet)
cd scripts/nu/setup-local-machine
nu shells.nu generate

# 7. Review generated configs
cat ../../output/zsh/.config/zsh/generated.zsh

# 8. Stow configs (if happy with generated output)
nu shells.nu stow

# 9. Test in a new shell
exec $SHELL
```

## Pre-Installation Checklist

Before running `./install.sh` on your system:

- [ ] Run `./test-install.sh` - all checks pass
- [ ] Run `./backup.sh` - backup created
- [ ] Review `brew/Brewfile` - packages you want
- [ ] Review `cargo-install.toml` - Cargo tools you want
- [ ] Review `global-packages.json` - npm/bun packages you want
- [ ] Review `scripts/nu/setup-local-machine/config.toml` - shell config
- [ ] Have backup restore script ready: `~/dotconfig-backup-*/restore.sh`

## Common Issues During Testing

### Issue: Stow conflicts

**Symptom:**
```
WARNING! stowing zsh would cause conflicts:
  * existing target is not owned by stow: .config/zsh
```

**Solution:**
```bash
# Backup and remove existing config
mv ~/.config/zsh ~/.config/zsh.backup

# Or incorporate it into dotconfig
cp -r ~/.config/zsh ~/dotconfig/output/zsh/.config/
```

### Issue: Package already installed

**Symptom:**
```
brew install foo
Warning: foo is already installed
```

**Solution:** This is expected and safe. Brew will skip already-installed packages.

### Issue: Cargo package compilation fails

**Symptom:**
```
error: failed to compile `some-package`
```

**Solution:**
```bash
# cargo-binstall will try binary first, then compile
# If compilation fails, you can skip that package
# or investigate the specific compilation error
```

### Issue: Permission denied

**Symptom:**
```
Permission denied: /usr/local/...
```

**Solution:**
```bash
# Fix Homebrew permissions
sudo chown -R $(whoami) /usr/local/*
sudo chown -R $(whoami) /opt/homebrew/*  # Apple Silicon
```

## Automated Testing (CI)

You can also run automated tests in CI:

```yaml
# .github/workflows/test.yml
name: Test Installation

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run validation tests
        run: ./test-install.sh
      - name: Test in Docker
        run: |
          docker build -f Dockerfile.test -t dotconfig-test .
          docker run dotconfig-test ./install.sh
```

## Recovery

If something goes wrong:

### Restore from backup
```bash
~/dotconfig-backup-YYYYMMDD-HHMMSS/restore.sh
```

### Unstow configurations
```bash
cd ~/dotconfig/scripts/nu/setup-local-machine
nu shells.nu unstow
```

### Remove dotconfig entirely
```bash
# Unstow first
cd ~/dotconfig/scripts/nu/setup-local-machine
nu shells.nu unstow

# Remove repository
rm -rf ~/dotconfig

# Restore backup
~/dotconfig-backup-YYYYMMDD-HHMMSS/restore.sh
```

## Recommended Testing Flow

1. **Validate locally**: `./test-install.sh`
2. **Test in Docker**: `./test-docker.sh` → Option 2
3. **Create backup**: `./backup.sh`
4. **Install on your system**: `./install.sh`
5. **Test new shell**: `exec $SHELL`
6. **Verify everything works**

If anything fails at step 6, run restore:
```bash
~/dotconfig-backup-YYYYMMDD-HHMMSS/restore.sh
```

## Help

If you encounter issues:
1. Check the error message carefully
2. Review `INSTALLATION.md` for troubleshooting
3. Run `./test-install.sh` to identify problems
4. Restore from backup if needed
5. File an issue with the error output
