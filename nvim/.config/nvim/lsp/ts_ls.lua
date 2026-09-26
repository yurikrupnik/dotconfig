-- typescript-language-server (node global). Covers the bun/deno TS in
-- nushell/.config/nushell/scripts/devkit/ as well as Nx workspaces.
--
-- tsserver is not bundled with the language server: it resolves `typescript`
-- from the workspace's node_modules. There is deliberately no global fallback:
-- the global typescript (config/node/package.json, "*") is 7.x, the native Go
-- rewrite, which ships no tsserver.js. Files outside a JS project (a loose .ts
-- script) therefore get no LSP; add typescript to that project instead.

return {
    cmd = { 'typescript-language-server', '--stdio' },
    filetypes = { 'javascript', 'javascriptreact', 'typescript', 'typescriptreact' },
    root_markers = { 'tsconfig.json', 'jsconfig.json', 'package.json', 'bun.lockb', '.git' },
    init_options = { hostInfo = 'neovim' },
    settings = {
        -- Inlay hints are off by default in tsserver; turn on the useful subset.
        typescript = {
            inlayHints = {
                includeInlayParameterNameHints = 'literals',
                includeInlayFunctionLikeReturnTypeHints = true,
                includeInlayVariableTypeHints = false,
            },
        },
        javascript = {
            inlayHints = { includeInlayParameterNameHints = 'literals' },
        },
    },
}
