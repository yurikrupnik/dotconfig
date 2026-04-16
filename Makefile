.PHONY: install update generate stow unstow test clean help

# Default target
.DEFAULT_GOAL := help

## install: Bootstrap the entire system (fresh machine setup)
install:
	@./install.sh

## update: Update all packages and tools (FORMAT=json|yaml|toml|table)
update:
	@./update.sh $(FORMAT)

## update-dry: Dry-run update to test reporting (FORMAT=json|yaml|toml|table)
update-dry:
	@./update.sh --dry-run $(FORMAT)

## generate: Generate shell configurations from config.toml
generate:
	@cd scripts/nu/setup-local-machine && nu shells.nu generate

## stow: Apply generated configurations to home directory
stow:
	@cd scripts/nu/setup-local-machine && nu shells.nu stow

## unstow: Remove configurations from home directory
unstow:
	@cd scripts/nu/setup-local-machine && nu shells.nu unstow

## regen: Generate and stow configurations (shortcut)
regen: generate stow
	@echo "Configurations regenerated and applied"

## build: Build the dotconfig CLI tool
build:
	@cargo build --release

## test: Run all tests
test:
	@cargo test
	@just test-nu

## clean: Clean build artifacts
clean:
	@cargo clean
	@rm -rf target/

## brew-install: Install/update Homebrew packages
brew-install:
	@brew bundle --file=brew/Brewfile

## cargo-install: Install global Cargo packages (skips already-installed)
cargo-install:
	@command -v cargo-binstall >/dev/null 2>&1 || cargo install cargo-binstall
	@cargo_bin="$${CARGO_HOME:-$$HOME/.cargo}/bin"; \
	grep '^[a-z]' cargo-install.toml | while read pkg; do \
		alt="$${pkg%-cli}"; \
		if [ -f "$$cargo_bin/$$pkg" ] || [ -f "$$cargo_bin/$$alt" ]; then \
			echo "==> Cargo: $$pkg already installed, skipping"; \
		else \
			cargo binstall "$$pkg" --no-confirm || echo "Failed to install $$pkg"; \
		fi; \
	done

## npm-install: Install global npm/bun packages
npm-install:
	@if command -v bun >/dev/null 2>&1; then \
		bun install --global; \
	elif command -v npm >/dev/null 2>&1; then \
		npm install --global; \
	else \
		echo "Neither bun nor npm found"; \
		exit 1; \
	fi

## backup: Create backup of existing configurations
backup:
	@./backup.sh

## test: Run validation tests (no installation)
test-validation:
	@./test-install.sh

## test-docker: Test installation in Docker
test-docker:
	@./test-docker.sh

## help: Show this help message
help:
	@echo "Dotconfig Makefile"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@sed -n 's/^##//p' $(MAKEFILE_LIST) | column -t -s ':' | sed -e 's/^/ /'
