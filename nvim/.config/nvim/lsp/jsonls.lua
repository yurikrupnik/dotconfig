-- vscode-json-language-server (node global: vscode-langservers-extracted).
-- Schemas come from the file's own $schema key; add project schemas here if a
-- repo needs them.
return {
    cmd = { 'vscode-json-language-server', '--stdio' },
    filetypes = { 'json', 'jsonc' },
    root_markers = { 'package.json', '.git' },
    init_options = { provideFormatter = false }, -- prettier formats JSON
    settings = { json = { validate = { enable = true } } },
}
