# see https://www.youtube.com/watch?v=KBh8lM3jeeE&t=36s for more details
[[ -f $HOME/.config/zsh/generated.zsh ]] && source $HOME/.config/zsh/generated.zsh
# Nix!
# export NIX_CONF_DIR=$HOME/.config/nix
. "$HOME/.cargo/env"

bindkey -r "^G"

# Devbox
# DEVBOX_NO_PROMPT=true
# eval "$(devbox global shellenv --init-hook)"
# Brew
# export PATH=/opt/homebrew/bin:$PATH
eval "$(/opt/homebrew/bin/brew shellenv)"
eval "$(direnv hook zsh)"
# Starship
eval "$(starship init zsh)"
export STARSHIP_CONFIG=~/.config/starship/starship.toml
# Zoxide
eval "$(zoxide init --cmd cd zsh)"
# Mise
eval "$(mise activate bash)"
# export CARAPACE_BRIDGES='zsh,fish,bash,inshellisense' # optional
# zstyle ':completion:*' format $'\e[2;37mCompleting %d\e[m'
# source <(carapace _carapace)
# # Rust
# . "$HOME/.cargo/env"

# XDG Base Directory Specification
export XDG_CONFIG_HOME="$HOME/.config"

export LANG=en_US.UTF-8
export PATH="$HOME/go/bin:$PATH"
export PATH="/usr/local/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"

# Added by Toolbox App
export PATH="$PATH:/Users/yurikrupnik/Library/Application Support/JetBrains/Toolbox/scripts"
export EDITOR=zed
export KUBE_EDITOR=zed
# export TERMINAL=WarpTerminal

HISTSIDE=5000
SAVEHIST=$HISTSIDE
HISTDUP=erase
setopt appendhistory
setopt sharehistory
setopt hist_ignore_space
setopt hist_ignore_dups
setopt hist_ignore_all_dups
setopt hist_save_no_dups
setopt hist_find_no_dups

autoload -Uz compinit && compinit
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'

if [ -f '/opt/homebrew/share/google-cloud-sdk/path.zsh.inc' ]; then . '/opt/homebrew/share/google-cloud-sdk/path.zsh.inc'; fi
if [ -f '/opt/homebrew/share/google-cloud-sdk/completion.zsh.inc' ]; then . '/opt/homebrew/share/google-cloud-sdk/completion.zsh.inc'; fi
