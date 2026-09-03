-- pyright (node global) — types and navigation only. Linting and formatting are
-- ruff's job, so pyright's overlapping diagnostics stay off.
return {
    cmd = { 'pyright-langserver', '--stdio' },
    filetypes = { 'python' },
    root_markers = { 'pyproject.toml', 'uv.lock', 'setup.py', 'requirements.txt', '.git' },
    settings = {
        python = {
            analysis = {
                typeCheckingMode = 'basic',
                autoSearchPaths = true,
                useLibraryCodeForTypes = true,
                diagnosticSeverityOverrides = { reportUnusedVariable = 'none' },
            },
        },
    },
}
