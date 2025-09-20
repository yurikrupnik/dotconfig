# https://just.systems

default:
    just --list
#    cargo run --bin dotconfig -- -h --debug
# Up shits
up:
    nu ~/dotconfig/scripts/nu/index.nu compose up --file ~/projects/playground/manifests/dockers/compose.yaml
down:
    nu ~/dotconfig/scripts/nu/index.nu compose down --file ~/projects/playground/manifests/dockers/compose.yaml
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