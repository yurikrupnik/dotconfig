# Fish shell configuration
# Based on zsh setup - see https://www.youtube.com/watch?v=KBh8lM3jeeE&t=36s

# Source generated files
if test -f $HOME/.config/fish/generated_aliases.fish
    source $HOME/.config/fish/generated_aliases.fish
end

if test -f $HOME/.config/fish/generated_env.fish
    source $HOME/.config/fish/generated_env.fish
end

# Cargo environment
if test -f $HOME/.cargo/env.fish
    source $HOME/.cargo/env.fish
end

# Homebrew
if test -f /opt/homebrew/bin/brew
    eval (/opt/homebrew/bin/brew shellenv)
end

# Direnv
if type -q direnv
    direnv hook fish | source
end

# Starship prompt
if type -q starship
    starship init fish | source
    set -gx STARSHIP_CONFIG ~/.config/starship/starship.toml
end

# Zoxide (smart cd)
if type -q zoxide
    zoxide init --cmd cd fish | source
end

# XDG Base Directory Specification
set -gx XDG_CONFIG_HOME $HOME/.config

# Language and locale
set -gx LANG en_US.UTF-8

# PATH additions
fish_add_path $HOME/go/bin
fish_add_path /usr/local/bin
fish_add_path $HOME/.local/bin
fish_add_path $HOME/.krew/bin

# JetBrains Toolbox
fish_add_path "$HOME/Library/Application Support/JetBrains/Toolbox/scripts"

# Editor
set -gx EDITOR zed
set -gx KUBE_EDITOR zed

# Google Cloud SDK
if test -f /opt/homebrew/share/google-cloud-sdk/path.fish.inc
    source /opt/homebrew/share/google-cloud-sdk/path.fish.inc
end

# Added by LM Studio CLI (lms)
set -gx PATH $PATH /Users/yurikrupnik/.lmstudio/bin
# End of LM Studio CLI section

