# Quick Test Guide

## Safe Testing Options (No Risk to Your System)

### Option 1: Validation Test (Fastest, Safest)
```bash
./test-install.sh
```
**What it does:**
- Validates all configuration files
- Checks syntax
- Shows what would be installed
- **Makes NO changes to your system**

---

### Option 2: Docker Test (Complete Isolation)
```bash
./test-docker.sh
```
**What it does:**
- Builds a clean Ubuntu container
- Tests installation in complete isolation
- Choose to run validation or full install
- **Zero risk to your system**

**Interactive menu:**
1. Run validation tests only
2. Run full installation in container
3. Start interactive shell (test manually)

---

### Option 3: Create Backup First
```bash
# 1. Create backup
./backup.sh

# 2. Review what will be backed up
ls -la ~/dotconfig-backup-*/

# 3. Run installation
./install.sh

# 4. If something breaks, restore:
~/dotconfig-backup-*/restore.sh
```

---

## Using Make Commands

```bash
make test-validation    # Run validation tests
make test-docker        # Run Docker tests
make backup             # Create backup
make install            # Full installation
```

---

## Recommended Testing Flow

### For Complete Safety:
```bash
# Step 1: Validate (0 risk)
./test-install.sh

# Step 2: Test in Docker (0 risk)
./test-docker.sh

# Step 3: Create backup (safe recovery)
./backup.sh

# Step 4: Install on your system
./install.sh
```

### Quick Validation:
```bash
./test-install.sh
```

### Full Docker Test:
```bash
docker build -f Dockerfile.test -t dotconfig-test .
docker run -it --rm dotconfig-test ./install.sh
```

---

## What Each Test Shows

### test-install.sh
```
✓ Checks all required files exist
✓ Validates script permissions
✓ Validates Brewfile syntax
✓ Counts packages in cargo-install.toml
✓ Validates JSON syntax
✓ Checks Nu scripts
✓ Shows installation summary
✓ Checks for potential conflicts
```

### test-docker.sh
```
✓ Builds isolated container
✓ Tests on clean Ubuntu system
✓ Full installation from scratch
✓ No impact on your system
```

### backup.sh
```
✓ Backs up existing configs
✓ Creates restore script
✓ Timestamped backup directory
✓ Easy one-command restore
```

---

## Files Created

- `test-install.sh` - Validation script (no installation)
- `test-docker.sh` - Docker test runner
- `backup.sh` - Backup script
- `Dockerfile.test` - Docker test environment
- `TESTING.md` - Comprehensive testing guide

---

## Quick Reference

| Command | Risk Level | What It Does |
|---------|------------|--------------|
| `./test-install.sh` | None | Validates only |
| `./test-docker.sh` | None | Tests in Docker |
| `./backup.sh` | None | Creates backup |
| `make test-validation` | None | Same as test-install.sh |
| `make test-docker` | None | Same as test-docker.sh |
| `make backup` | None | Same as backup.sh |
| `./install.sh` | Medium | Installs on your system |

---

## Need More Details?

See `TESTING.md` for comprehensive testing documentation including:
- Detailed testing methods
- Testing individual components
- Troubleshooting common issues
- Recovery procedures
- CI/CD integration
