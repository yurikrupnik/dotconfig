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

# proto
export PROTO_HOME="$HOME/.proto";
export PATH="$PROTO_HOME/shims:$PROTO_HOME/bin:$PATH";

export PATH="$HOME/go/bin:$PATH"
export PATH="/usr/local/bin:$PATH"
export PATH="$HOME/.local/bin:$PATH"
export PATH="$HOME/.bun/bin:$PATH"
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"

# Added by Toolbox App (per-machine; guarded so it's a no-op when JetBrains absent)
JETBRAINS_SCRIPTS="$HOME/Library/Application Support/JetBrains/Toolbox/scripts"
[ -d "$JETBRAINS_SCRIPTS" ] && export PATH="$PATH:$JETBRAINS_SCRIPTS"
# export TERMINAL=WarpTerminal

HISTSIZE=5000
SAVEHIST=$HISTSIZE
HISTDUP=erase
setopt appendhistory
setopt sharehistory
setopt hist_ignore_space
setopt hist_ignore_dups
setopt hist_ignore_all_dups
setopt hist_save_no_dups
setopt hist_find_no_dups

# Run log for `toolbelt` (config/scripts/toolbelt.nu): the settings above keep
# one copy of each command line, so history can't say how OFTEN a tool runs.
# One line per command: <epoch-seconds>\t<command>. Space-prefixed commands are
# skipped, same as hist_ignore_space. Builtins only — no fork per command.
zmodload -F zsh/datetime p:EPOCHSECONDS
_toolbelt_log_file="${XDG_STATE_HOME:-$HOME/.local/state}/toolbelt/zsh.tsv"
[[ -f $_toolbelt_log_file ]] || { mkdir -p "${_toolbelt_log_file:h}" && : >> "$_toolbelt_log_file" && chmod 600 "$_toolbelt_log_file"; }
_toolbelt_log() {
    [[ $1 == ' '* ]] && return
    local cmd=${1//$'\n'/ }
    print -r -- "$EPOCHSECONDS"$'\t'"${cmd//$'\t'/ }" >> "$_toolbelt_log_file"
}
autoload -Uz add-zsh-hook
add-zsh-hook preexec _toolbelt_log

autoload -Uz compinit && compinit
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}'

if [ -f '/opt/homebrew/share/google-cloud-sdk/path.zsh.inc' ]; then . '/opt/homebrew/share/google-cloud-sdk/path.zsh.inc'; fi
if [ -f '/opt/homebrew/share/google-cloud-sdk/completion.zsh.inc' ]; then . '/opt/homebrew/share/google-cloud-sdk/completion.zsh.inc'; fi
