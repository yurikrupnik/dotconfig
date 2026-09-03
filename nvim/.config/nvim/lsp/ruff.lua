-- ruff (brew) as a language server: lint diagnostics + quick fixes + organize
-- imports. Hover is disabled so pyright owns hover documentation.
return {
    cmd = { 'ruff', 'server' },
    filetypes = { 'python' },
    root_markers = { 'pyproject.toml', 'ruff.toml', '.ruff.toml', 'uv.lock', '.git' },
    on_attach = function(client)
        client.server_capabilities.hoverProvider = false
    end,
}
