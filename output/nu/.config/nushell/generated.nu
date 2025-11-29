# Generated from config.toml

# Aliases
export alias lg = lazygit
export alias k = kubectl
export alias lsl = eza --no-permissions --no-user --no-time --long
export def drmi [] {
    ^docker rmi (^docker images -aq | str trim) -f
}
export def dclean [] {
    ^docker system prune -f
    ^docker volume prune -f
}
export alias b = bun
export alias c = cargo
export alias cdoc = cargo doc
export alias gs = git status
export alias ga = git add
export alias claude = bun run ~/.bun/bin/claude

# Functions
# Update system packages
export def u [...args] {
    ^brew update
    ^brew bundle --file ~/dotconfig/brew/Brewfile --upgrade
    ^rustup update
    ^gcloud components update
    ^npm update -g
    ^nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate ...$args
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
