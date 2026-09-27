# https://just.systems
# Dotfiles task runner. For details on what each script does, see README.md.

set shell := ["bash", "-cu"]

shells := "nu scripts/nu/setup-local-machine/shells.nu"

default:
    @just --list

# devkit lives in the toolkit repo (see AGENTS.md); this repo only holds devkit.toml
devkit:
    devkit

# Local platform via devkit. Not named `up`: that shadows the Upbound CLI.
dev-up:
    devkit up --istio --core --gitops --observability --flux

# Run CI (lint/generator/secrets from ci.yml) as a Tekton PipelineRun in the
# devkit Kind cluster, against the working tree. Needs `devkit cluster create`
# and `devkit cluster deps` (Tekton comes from devkit.toml [[deps]]).
ci-tekton *args:
    nu scripts/nu/ci-tekton.nu {{ args }}

# Package-manager network traffic goes through Socket Firewall. Interactive
# shells get it from the sfw aliases in config.toml; scripts don't expand
# aliases, so recipes call `sfw` explicitly. install.sh bootstraps sfw first.
[private]
require-sfw:
    @command -v sfw >/dev/null || { echo "sfw not on PATH; bootstrap it once: bun add --global sfw" >&2; exit 1; }

# Fresh-machine bootstrap (brew, rust, cargo-liner, shells, stow)
install:
    ./install.sh

# Generate shell configs and bin/ scripts from config/
generate:
    {{ shells }} generate

# Symlink generated configs into $HOME (via GNU stow)
stow:
    {{ shells }} stow

# Remove symlinked configs from $HOME
unstow:
    {{ shells }} unstow

# Generate + stow in one step
regen: generate stow

# Show what stow would do without making changes
stow-dry:
    {{ shells }} stow --dry-run

# Preview Brewfile install: counts + which taps need trust (read-only)
brew-preflight:
    ./scripts/brew-preflight.sh --check

# Update brew packages from Brewfile (preflight → trust new taps → bundle)
brew-install:
    ./scripts/brew-preflight.sh --apply

# Install/update global cargo packages via cargo-liner (cargo-binstall comes from the Brewfile)
cargo-install: require-sfw
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v cargo-binstall &> /dev/null; then
        echo "cargo-binstall missing; it is declared in the Brewfile: just brew-install" >&2
        exit 1
    fi
    # sfw MITMs TLS with a per-run CA it exports as SSL_CERT_FILE. cargo honors
    # it (CARGO_HTTP_CAINFO); cargo-binstall only trusts its bundled roots unless
    # given BINSTALL_HTTPS_ROOT_CERTS, and fails with UnknownIssuer otherwise.
    sfw_cargo() { sfw bash -c 'BINSTALL_HTTPS_ROOT_CERTS="$SSL_CERT_FILE" exec cargo "$@"' _ "$@"; }
    if ! command -v cargo-liner &> /dev/null; then
        echo "==> Installing cargo-liner..."
        sfw_cargo binstall cargo-liner --no-confirm
    fi
    liner_src="{{ justfile_directory() }}/config/cargo/liner.toml"
    liner_dest="${CARGO_HOME:-$HOME/.cargo}/liner.toml"
    if [ ! -L "$liner_dest" ] || [ "$(readlink "$liner_dest")" != "$liner_src" ]; then
        echo "==> Linking cargo-liner config: $liner_dest -> $liner_src"
        ln -sfn "$liner_src" "$liner_dest"
    fi
    sfw_cargo liner ship

# Install global node packages declared in config/node/package.json (writes their
# ranges into bun's global manifest, so a later `bun update --global` honors them)
node-install: require-sfw
    cd config/node && sfw bun add --global $(jq -r '.dependencies | to_entries[] | "\(.key)@\(.value)"' package.json)

# Install global Python CLI tools declared in config/uv/tools.txt
uv-install: require-sfw
    awk '!/^[[:space:]]*(#|$)/ {print $1}' config/uv/tools.txt | while IFS= read -r pkg; do sfw uv tool install "$pkg" || echo "  ! uv tool install $pkg failed"; done

# Verify the install is healthy (commands, symlinks, freshness)
doctor:
    ./scripts/doctor.sh

# Preview what `u` would refresh (read-only)
outdated:
    ./scripts/outdated.sh

# Disposable Ubuntu VM (Lima): ./install.sh on the working tree, then a shell.
# Flags: --keep (reuse next run) --fresh --shell --template <lima template>
sandbox-linux *args:
    nu scripts/nu/sandbox.nu linux {{ args }}

# Disposable macOS VM (Lima, Apple Silicon); same flags as sandbox-linux
sandbox-mac *args:
    nu scripts/nu/sandbox.nu mac {{ args }}

# Delete every sandbox VM, including the cached pristine bases
sandbox-clean:
    nu scripts/nu/sandbox.nu clean

runs:
    just doctor # fails

