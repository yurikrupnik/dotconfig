# https://just.systems

default:
    just --list
#    cargo run --bin dotconfig -- -h --debug
# Up shits
up:
    nu ~/dotconfig/scripts/nu/index.nu compose up --file ~/projects/playground/manifests/dockers/compose.yaml
down:
    nu ~/dotconfig/scripts/nu/index.nu compose down --file ~/projects/playground/manifests/dockers/compose.yaml
    docker compose -f docker-compose.dev.yml -f ~/projects/playground/manifests/dockers/compose.yaml up
rcli:
    cargo run --bin dotconfig -- -h
    cargo run --bin dotconfig compose -h
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
    @echo "🔧 Creating minimal cluster: {{name}}"
    @echo "🔧 Creating minimal cluster: {{providers}}"