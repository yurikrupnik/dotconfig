# https://just.systems

#default:
#    just --list
#    cargo run --bin dotconfig -- -h --debug
car:
    cargo run --bin resource-stats-operator -- collect
    cargo run --features full --bin resource-stats-operator -- crds
    just up
up *args='':
  nu ~/dotconfig/scripts/nu/index.nu dev up {{args}}
down *args='':
    nu ~/dotconfig/scripts/nu/index.nu dev down {{args}}
rcli:
    cargo run --bin dotconfig -- -h
    cargo run --bin dotconfig compose -h
bun-ag:
    bun add -g @anthropic-ai/claude-code @openai/codex
    bun pm ls -g
    bun update -g
    bun run ~/.bun/bin/claude
    bun run ~/.bun/bin/claude
#com:up
#    cargo run --bin dotconfig compose up -h
#com:down
#    cargo run --bin dotconfig compose down -h
#com:convert
#    cargo run --bin dotconfig compose convert -h
create-env: up rcli
    #just up
destroy-env: down rcli
    #just up

# Test commands
test-rust:
    cargo test --all
    cargo release
    cargo audit --json
    cargo check
test-nu:
    nu scripts/nu/tests/resolve_compose_files_test.nu
test: test-rust test-nu
    cargo modules structure
    echo "All tests passed!"

# Toolchain management
toolchain-install:
    ./.cargo/bin/toolchain-install.sh

toolchain-update:
    ./.cargo/bin/toolchain-update.sh

toolchain-targets:
    ./.cargo/bin/toolchain-ensure-targets.sh

toolchain-info:
    ./.cargo/bin/toolchain-info.sh

# One-time setup on a new machine
setup-rust:
    ./.cargo/bin/rustup-bootstrap.sh
    just toolchain-install
    just toolchain-info
cargo run:
    cargo run --bin dotconfig shit do-it -n aris
    cargo run --bin operator
# CI-ready setup
ci-rust-setup:
    just setup-rust
# Create minimal cluster (no additional components)
create-cluster-minimal name="minimal" providers="aws,gcp":
    nu ~/dotconfig/scripts/nu/index.nu -h
    nu ~/dotconfig/scripts/nu/index.nu kcl init --path scripts/kcl/stam
    @echo "Creating minimal cluster: {{name}}"
    @echo "Creating minimal cluster: {{providers}}"

# =============================================================================
# API Server
# =============================================================================

# Run API server locally
api-dev:
    cd apps/api && cargo run --bin api-server

# Run API server with hot reload
api-watch:
    cd apps/api && cargo watch -x 'run --bin api-server'

# Build API server
api-build:
    cd apps/api && cargo build --release --bin api-server

# Test API server
api-test:
    cd apps/api && cargo test

# Build API Docker image
api-docker-build tag="latest":
    docker build -t ghcr.io/yurikrupnik/api-server:{{tag}} -f apps/api/Dockerfile .

# Deploy API to K8s
api-k8s-deploy:
    kubectl apply -k apps/api/k8s/

# Delete API from K8s
api-k8s-delete:
    kubectl delete -k apps/api/k8s/ --ignore-not-found

# =============================================================================
# Tauri Apps
# =============================================================================

# Run web-app (Leptos + Tauri)
tauri-web-dev:
    cd web-app && cargo tauri dev

# Run native-app (SolidJS + Tauri)
tauri-native-dev:
    cd app/native-app && cargo tauri dev

# Build web-app for desktop
tauri-web-build:
    cd web-app && cargo tauri build

# Build native-app for desktop
tauri-native-build:
    cd app/native-app && cargo tauri build

# =============================================================================
# Full Stack Development
# =============================================================================

# Run API + web-app frontend (browser mode)
dev-web: api-dev
    cd web-app && trunk serve

# Run API + native-app frontend (browser mode)
dev-native: api-dev
    cd app/native-app && bun run dev
