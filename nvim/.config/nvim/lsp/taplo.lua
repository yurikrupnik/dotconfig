-- taplo (brew) — TOML: Cargo.toml, config/shell/config.toml, liner.toml, bunfig.
return {
    cmd = { 'taplo', 'lsp', 'stdio' },
    filetypes = { 'toml' },
    root_markers = { 'Cargo.toml', '.taplo.toml', 'taplo.toml', '.git' },
}
