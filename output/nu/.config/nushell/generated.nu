# Generated from config.toml

# Aliases
export alias lg = lazygit
export alias k = kubectl
export alias lsl = eza --no-permissions --no-user --no-time --long
export def drmi [] {
    ^docker rmi (^docker images -aq | str trim) -f
}
export def dsp [] {
    ^docker system prune -f
    ^docker volume prune -f
}
export alias pu = pulumi
export alias b = bun
export alias c = cargo
export alias cdo = cargo doc
export alias gs = git status
export alias ga = git add
export alias g = gcloud
export alias gal = gcloud auth login

# Functions
# Refresh installed packages on this machine
export def u [] {
    ^brew update
    ^brew bundle --file ~/dotconfig/config/brew/Brewfile
    ^brew upgrade
    ^brew cleanup
    ^rustup update
    ^cargo liner ship --no-fail-fast
    ^cargo install-update -a
    ^bun update --global
    ^gcloud components update --quiet
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu stow
}

export def sort [] {
    ^cargo fmt
    ^cargo sort --workspace
}

# Run generate shells commands, supports zsh, fish and nu scripts.
export def generate [...args] {
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate ...$args
}

# Run Nx command on all projects
export def nx-run [...args] {
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/nx.nu ...$args
}

# Generate MCP server configuration file
export def mcp [...args] {
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/mcp.nu ...$args
}

# Environment Variables
$env.EDITOR = 'zed'
$env.BROWSER = 'open'
$env.SHIT_TEXT = 'omg'
$env.SHIT = true
$env.AI = 'claude'
$env.PROJECT = 'rust'
$env.TYPE = 'be'
$env.CLOUD_GPROJECT = 'playground-447016'
$env.CLOUD_GACOUNT = 'krupnik.yuri@gmail.com'
$env.CLOUD_GREGION = 'me-west1'
$env.CLOUD = 'gcp'
$env.CONTAINER_REGISTRY = 'docker.com/yurikrupnik'
$env.CONTAINER_REGISTRY_BACKUP = 'me-west1-docker.pkg.dev/playground-447016/containers'
