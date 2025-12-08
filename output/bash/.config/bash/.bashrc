# Bash shell configuration
# Based on zsh setup - see https://www.youtube.com/watch?v=KBh8lM3jeeE&t=36s

# Source generated files
[[ -f $HOME/.config/bash/generated.bash ]] && source $HOME/.config/bash/generated.bash

# Cargo environment
[[ -f $HOME/.cargo/env ]] && . "$HOME/.cargo/env"

# Homebrew
if [[ -f /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
fi

# Direnv
if command -v direnv &> /dev/null; then
    eval "$(direnv hook bash)"
fi

# Starship prompt
if command -v starship &> /dev/null; then
    eval "$(starship init bash)"
    export STARSHIP_CONFIG=~/.config/starship/starship.toml
fi

# Zoxide (smart cd)
if command -v zoxide &> /dev/null; then
    eval "$(zoxide init --cmd cd bash)"
fi

# XDG Base Directory Specification
export XDG_CONFIG_HOME="$HOME/.config"

# Language and locale
export LANG=en_US.UTF-8

# PATH additions
export PATH="$HOME/go/bin:$PATH"
export PATH="/usr/local/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"

# JetBrains Toolbox
export PATH="$PATH:$HOME/Library/Application Support/JetBrains/Toolbox/scripts"

# Editor
export EDITOR=zed
export KUBE_EDITOR=zed

# History configuration
HISTFILE="$HOME/dotconfig/output/bash/.config/bash/.bash_history"
HISTSIZE=5000
HISTFILESIZE=$HISTSIZE
HISTCONTROL=ignorespace:ignoredups:erasedups
shopt -s histappend

# Google Cloud SDK
if [[ -f /opt/homebrew/share/google-cloud-sdk/path.bash.inc ]]; then
    . '/opt/homebrew/share/google-cloud-sdk/path.bash.inc'
fi
if [[ -f /opt/homebrew/share/google-cloud-sdk/completion.bash.inc ]]; then
    . '/opt/homebrew/share/google-cloud-sdk/completion.bash.inc'
fi
