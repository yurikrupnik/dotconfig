#!/usr/bin/env nu

# Project Setup Script
# Install dependencies, build, and configure the monorepo

use common.nu *
use config.nu *

# Install system dependencies
export def "devkit setup install" [
    --rust       # Install Rust toolchain and cargo tools
    --node       # Install Node.js/Bun dependencies
    --k8s        # Install Kubernetes tools
    --all (-a)   # Install everything
] {
    info "Installing system dependencies..."

    let install_all = $all or (not $rust and not $node and not $k8s)

    if is-macos {
        install-macos-deps ($install_all or $rust) ($install_all or $node) ($install_all or $k8s)
    } else if is-linux {
        install-linux-deps ($install_all or $rust) ($install_all or $node) ($install_all or $k8s)
    } else {
        warn "Unsupported OS. Please install dependencies manually."
    }

    success "Dependencies installed!"
}

# macOS dependency installation
def install-macos-deps [rust: bool, node: bool, k8s: bool] {
    if not (command-exists "brew") {
        error "Homebrew not found. Install from https://brew.sh"
        exit 1
    }

    if $rust {
        info "Installing Rust dependencies..."
        let rust_tools = ["rust"]
        for tool in $rust_tools {
            if not (command-exists $tool) {
                brew install $tool
            }
        }

        # Rust components
        rustup component add clippy rustfmt

        # Cargo tools
        let cargo_tools = ["cargo-watch" "cargo-nextest" "sqlx-cli" "cargo-sort"]
        for tool in $cargo_tools {
            let bin_name = ($tool | str replace "cargo-" "")
            if not (command-exists $bin_name) and not (command-exists $tool) {
                info $"Installing ($tool)..."
                cargo install $tool
            }
        }
    }

    if $node {
        info "Installing Node/Bun dependencies..."
        if not (command-exists "bun") {
            brew install bun
        }
        bun install
    }

    if $k8s {
        info "Installing Kubernetes tools..."
        let k8s_tools = ["vals" "vault" "kubectl" "kustomize" "tilt" "kind" "helm" "flux" "kcl"]
        for tool in $k8s_tools {
            if not (command-exists $tool) {
                info $"Installing ($tool)..."
                brew install $tool
            }
        }
    }
}

# Linux dependency installation
def install-linux-deps [rust: bool, node: bool, k8s: bool] {
    if $rust {
        if (command-exists "apt-get") {
            sudo apt-get update
            sudo apt-get install -y curl build-essential pkg-config libssl-dev
        }

        if not (command-exists "rustc") {
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        }

        rustup component add clippy rustfmt
    }

    if $node {
        if not (command-exists "bun") {
            curl -fsSL https://bun.sh/install | bash
        }
        bun install
    }

    if $k8s {
        # Install vals
        if not (command-exists "vals") {
            info "Installing vals..."
            let version = "0.37.1"
            curl -L $"https://github.com/helmfile/vals/releases/download/v($version)/vals_($version)_linux_amd64.tar.gz" | tar xz -C /tmp
            sudo mv /tmp/vals /usr/local/bin/
        }
    }
}

# Build all Rust packages
export def "devkit setup build" [
    --release (-r)     # Build in release mode
    --app (-a): string # Build specific app
] {
    require-bin "cargo"

    if ($app | is-not-empty) {
        info $"Building ($app)..."
        if $release {
            cargo build --release -p $app
        } else {
            cargo build -p $app
        }
    } else {
        info "Building all Rust packages..."
        if $release {
            cargo build --release
        } else {
            cargo build
        }
    }

    success "Build complete!"
}

# Run quality checks
export def "devkit setup check" [
    --fix       # Auto-fix issues where possible
] {
    require-bin "cargo"

    info "Running quality checks..."

    if $fix {
        cargo fmt --all
        cargo clippy --workspace --all-targets --fix --allow-dirty
    } else {
        cargo fmt --all --check
        cargo clippy --workspace --all-targets -- -D warnings
    }

    success "Checks passed!"
}

# Run tests
export def "devkit setup test" [
    --watch (-w)  # Watch mode
] {
    require-bin "cargo"

    if $watch {
        require-bin "cargo-watch"
        cargo watch -x "nextest run"
    } else {
        cargo nextest run --workspace
    }
}

# Setup Vault secrets structure
export def "devkit setup vault-setup" [] {
    require-bin "vault"

    info "Setting up Vault secrets structure..."

    let vault_addr = ($env.VAULT_ADDR? | default "http://localhost:8200")
    with-env { VAULT_ADDR: $vault_addr } {
        # Enable KV secrets engine
        let kv_result = (do { vault secrets enable -path=secret kv-v2 } | complete)
        if $kv_result.exit_code == 0 {
            success "KV secrets engine enabled"
        } else {
            info "KV secrets engine already enabled"
        }

        # Create example secrets
        # NOTE: these secret/zerg/* paths are repo-specific examples, not part of the devkit config schema
        vault kv put secret/zerg/database DATABASE_USER=zerg_user DATABASE_PASSWORD=change-me DATABASE_NAME=zerg
        vault kv put secret/zerg/auth JWT_SECRET=your-jwt-secret-min-32-characters
        vault kv put secret/zerg/email SENDGRID_API_KEY=SG.your-key

        success "Vault secrets structure created!"
        warn "Remember to update placeholder values with real secrets"
    }
}

# Setup Kubernetes resources
export def "devkit setup k8s-setup" [] {
    require-bin "kubectl"
    require-bin "helm"

    info "Setting up Kubernetes resources..."

    let ns = (resolve-config).namespaces.external_secrets

    # Create namespace
    #kubectl create namespace zerg --dry-run=client -o yaml | kubectl apply -f -

    # Install External Secrets Operator
    let eso_installed = (do { kubectl get deployment -n $ns external-secrets } | complete).exit_code == 0

    if not $eso_installed {
        info "Installing External Secrets Operator..."
        helm repo add external-secrets https://charts.external-secrets.io
        helm repo update
        helm install external-secrets external-secrets/external-secrets -n $ns --create-namespace
    }

    success "Kubernetes resources configured!"
}

# Full setup
export def "devkit setup all" [] {
    info "Running full setup..."

    devkit setup install --all
    devkit setup build --release

    success "Full setup complete!"
    info "Run 'devkit dev up -d' to start services"
}
