# Generated from config.toml

# Aliases
alias lg='lazygit'
alias k='kubectl'
alias lsl='eza --no-permissions --no-user --no-time --long'
alias u='update'
alias drmi='docker rmi $(docker images -aq) -f'
alias dsp='docker system prune -f && docker volume prune -f'
alias pu='pulumi'
alias b='bun'
alias c='cargo'
alias cdo='cargo doc'
alias gs='git status'
alias ga='git add'
alias g='gcloud'
alias gal='gcloud auth login'

# Functions
update() {
    brew update
    brew bundle --file ~/dotconfig/config/brew/Brewfile
    brew upgrade
    brew cleanup
    rustup update
    cargo liner ship --no-fail-fast
    cargo install-update -a
    bun update --global
    gcloud components update --quiet
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu stow
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
