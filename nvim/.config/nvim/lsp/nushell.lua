-- Nushell's own language server, shipped inside the `nu` binary (brew nushell).
-- Gives completions and hover for the devkit modules and config/scripts/*.nu.
return {
    cmd = { 'nu', '--lsp' },
    filetypes = { 'nu' },
    root_markers = { '.git' },
}
