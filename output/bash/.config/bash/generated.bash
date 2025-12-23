# Generated from config.toml

# Aliases
alias lg='lazygit'
alias k='kubectl'
alias lsl='eza --no-permissions --no-user --no-time --long'
alias u='update'
alias drmi='docker rmi $(docker images -aq) -f'
alias dclean='docker system prune -f && docker volume prune -f'
alias b='bun'
alias c='cargo'
alias cdoc='cargo doc'
alias gs='git status'
alias ga='git add'

# Functions
update() {
    brew update
    brew bundle --file ~/dotconfig/brew/Brewfile --upgrade
    brew bundle cleanup --file ~/dotconfig/brew/Brewfile --force
    brew cleanup
    rustup update
    cargo install-update -a
    gcloud components update
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate "$@"
}

sort() {
    cargo fmt
    cargo sort --workspace
}

generate() {
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate "$@"
}

nx-run() {
    nu ~/dotconfig/scripts/nu/setup-local-machine/nx.nu "$@"
}

mcp() {
    nu ~/dotconfig/scripts/nu/setup-local-machine/mcp.nu "$@"
}

# Environment Variables
export EDITOR='zed'
export BROWSER='open'
export SHIT_TEXT='omg'
export SHIT='true'
export AI='claude'
export PROJECT='rust'
export TYPE='be'
export CLOUD_GPROJECT='playground-447016'
export CLOUD_GACOUNT='krupnik.yuri@gmail.com'
export CLOUD_GREGION='me-west1'
export CLOUD='gcp'
export CONTAINER_REGISTRY='docker.com/yurikrupnik'
export CONTAINER_REGISTRY_BACKUP='me-west1-docker.pkg.dev/playground-447016/containers'
