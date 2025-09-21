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
    cargo audit
test-nu:
    nu scripts/nu/tests/resolve_compose_files_test.nu
test: test-rust test-nu
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

# CI-ready setup
ci-rust-setup:
    just setup-rust
