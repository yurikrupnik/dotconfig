#!/usr/bin/env bash
# devkit — cross-shell launcher for the Nushell `devkit` module.
#
# Installed to ~/.local/bin/devkit by the dotfiles generator (extension stripped).
# Runs the module in a nu subshell so `devkit ...` works from bash/zsh/any shell.
# The module lives at ~/.config/nushell/scripts/devkit (auto on NU_LIB_DIRS), so
# no path wiring is needed. Per-repo devkit.toml is discovered from $PWD.
#
#   devkit up
#   devkit cluster create -n dev -w 2
#   devkit config
set -euo pipefail

if ! command -v nu >/dev/null 2>&1; then
    echo "devkit: nushell (nu) not found on PATH" >&2
    exit 127
fi

# With no args, show the curated overview (the module's `main` command).
if [[ $# -eq 0 ]]; then
    exec nu -c 'use devkit *; devkit'
fi

# Build a single nu command line, forwarding args. Nushell parses flags
# (--dry-run) only when they are BARE tokens, so we leave args untouched unless
# they contain characters that would break nu's tokenizer. Then:
#   - no single quote  -> wrap in nu single-quotes (verbatim, no escapes)
#   - has single quote -> wrap in nu double-quotes, escaping \ and " (and $ `)
# This keeps flags as flags and preserves values with spaces/globs/quotes.
nu_quote() {
    local s=$1
    if [[ $s =~ ^[A-Za-z0-9_./:=@%+,-]+$ ]]; then
        printf '%s' "$s"
    elif [[ $s != *\'* ]]; then
        printf "'%s'" "$s"
    else
        # Escape backslash, double-quote, dollar and backtick for nu double-quotes.
        local e=$s
        e=${e//\\/\\\\}
        e=${e//\"/\\\"}
        e=${e//\$/\\\$}
        e=${e//\`/\\\`}
        printf '"%s"' "$e"
    fi
}

line="use devkit *; devkit"
for a in "$@"; do
    line+=" $(nu_quote "$a")"
done

exec nu -c "$line"
