# Nushell environment configuration
# Based on zsh setup - see https://www.youtube.com/watch?v=KBh8lM3jeeE&t=36s

# PATH setup
$env.PATH = ($env.PATH | split row (char esep) | prepend [
    $"($env.HOME)/go/bin"
    "/usr/local/bin"
    $"($env.HOME)/.local/bin"
    $"($env.HOME)/.bun/bin"
    $"($env.HOME)/.krew/bin"
    $"($env.HOME)/Library/Application Support/JetBrains/Toolbox/scripts"
])

# Homebrew
if ('/opt/homebrew/bin/brew' | path exists) {
    # Load Homebrew environment
    $env.PATH = ($env.PATH | prepend '/opt/homebrew/bin')
    $env.PATH = ($env.PATH | prepend '/opt/homebrew/sbin')
    $env.HOMEBREW_PREFIX = "/opt/homebrew"
    $env.HOMEBREW_CELLAR = "/opt/homebrew/Cellar"
    $env.HOMEBREW_REPOSITORY = "/opt/homebrew"
    $env.MANPATH = $"/opt/homebrew/share/man:($env.MANPATH? | default '')"
    $env.INFOPATH = $"/opt/homebrew/share/info:($env.INFOPATH? | default '')"
}

# Cargo (Rust)
if ($"($env.HOME)/.cargo/env" | path exists) {
    $env.PATH = ($env.PATH | prepend $"($env.HOME)/.cargo/bin")
}

# Starship prompt
if (which starship | is-not-empty) {
    $env.STARSHIP_CONFIG = $"($env.HOME)/.config/starship/starship.toml"
    mkdir ~/.cache/starship
    starship init nu | save -f ~/.cache/starship/init.nu
}

# Zoxide
if (which zoxide | is-not-empty) {
    mkdir ~/.cache/zoxide
    zoxide init nushell | save -f ~/.cache/zoxide/init.nu
}

# Direnv
if (which direnv | is-not-empty) {
    mkdir ~/.cache/direnv
    let direnv_hook = r#'
# Direnv integration for Nushell
# This sets up hooks to automatically load direnv when changing directories

def --env direnv-load [] {
    let result = (
        with-env { DIRENV_LOG_FORMAT: "" } {
            ^direnv export json
        }
    )

    if ($result == "") {
        return
    }

    try {
        $result | from json | load-env
    } catch {
        # Ignore errors if direnv returns invalid JSON
    }
}

# Load direnv for current directory on shell startup
direnv-load

# Set up hooks for future directory changes
$env.config = ($env.config | upsert hooks {|config|
    {
        pre_prompt: [
            {||
                direnv-load
            }
        ]
        env_change: {
            PWD: [
                {|before, after|
                    direnv-load
                }
            ]
        }
    }
})
'#
    $direnv_hook | save -f ~/.cache/direnv/init.nu
}

