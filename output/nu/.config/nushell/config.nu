# Nushell configuration
# Based on zsh setup - see https://www.youtube.com/watch?v=KBh8lM3jeeE&t=36s

# Shell behavior configuration
$env.config = {
    show_banner: false

    # History configuration (similar to zsh HISTSIZE, SAVEHIST settings)
    history: {
        max_size: 5000
        sync_on_enter: true
        file_format: "sqlite"
        isolation: false
    }

    # Completions (similar to zsh compinit with case-insensitive matching)
    completions: {
        case_sensitive: false
        quick: true
        partial: true
        algorithm: "fuzzy"
    }

    # File completions
    filesize: {
        metric: true
        format: "auto"
    }

    # Cursor shape
    cursor_shape: {
        emacs: line
        vi_insert: line
        vi_normal: block
    }

    # Edit mode
    edit_mode: emacs

    # Shell integration
    shell_integration: {
        osc2: true
        osc7: true
        osc8: true
        osc9_9: false
        osc133: true
        osc633: true
        reset_application_mode: true
    }

    # Table display
    table: {
        mode: rounded
        index_mode: auto
        show_empty: true
        trim: {
            methodology: wrapping
            wrapping_try_keep_words: true
        }
    }

    # Error display
    error_style: "fancy"

    # Use system clipboard for copy/paste
    use_ansi_coloring: true
    bracketed_paste: true
}

# Source shell integrations
# Starship prompt
if ('~/.cache/starship/init.nu' | path expand | path exists) {
    use ~/.cache/starship/init.nu
}

# Zoxide (smart cd)
if ('~/.cache/zoxide/init.nu' | path expand | path exists) {
    use ~/.cache/zoxide/init.nu
}

# Direnv
if ('~/.cache/direnv/init.nu' | path expand | path exists) {
    use ~/.cache/direnv/init.nu
}

# Source generated configuration (aliases, functions, environment variables)
if ('~/.config/nushell/generated.nu' | path expand | path exists) {
    use ~/.config/nushell/generated.nu *
}

# Keybindings
# Unbind Ctrl+G (similar to zsh bindkey -r "^G")
# Note: Nushell keybindings are configured differently than zsh
