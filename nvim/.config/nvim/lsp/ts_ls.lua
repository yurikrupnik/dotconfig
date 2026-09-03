-- typescript-language-server (node global). Covers the bun/deno TS in
-- nushell/.config/nushell/scripts/devkit/ as well as Nx workspaces.
--
-- tsserver is not bundled with the language server: it resolves `typescript`
-- from the workspace's node_modules. Files outside a JS project (a loose .ts
-- script, a bun single-file server) have no node_modules, and the server exits
-- with "Could not find a valid TypeScript installation". before_init below
-- points it at the global typescript in exactly that case, so a project-pinned
-- TypeScript version always wins.
--
-- This is why config/node/package.json pins "typescript": "6.*": typescript@7
-- is the native (Go) rewrite and ships no tsserver.js at all, so it cannot back
-- typescript-language-server. Projects with their own TypeScript are unaffected.
local function global_tsserver_lib()
    local candidates = {
        vim.fs.joinpath(vim.env.BUN_INSTALL or (vim.env.HOME .. '/.bun'),
            'install/global/node_modules/typescript/lib'),
        '/opt/homebrew/lib/node_modules/typescript/lib',
        '/usr/local/lib/node_modules/typescript/lib',
    }
    for _, dir in ipairs(candidates) do
        if vim.fn.isdirectory(dir) == 1 then
            return dir
        end
    end
end

return {
    cmd = { 'typescript-language-server', '--stdio' },
    filetypes = { 'javascript', 'javascriptreact', 'typescript', 'typescriptreact' },
    root_markers = { 'tsconfig.json', 'jsconfig.json', 'package.json', 'bun.lockb', '.git' },
    init_options = { hostInfo = 'neovim' },
    before_init = function(params, config)
        local root = config.root_dir or vim.uv.cwd()
        if vim.fn.isdirectory(vim.fs.joinpath(root, 'node_modules/typescript/lib')) == 1 then
            return -- workspace has its own TypeScript; use it
        end
        local lib = global_tsserver_lib()
        if lib then
            params.initializationOptions = vim.tbl_deep_extend('force',
                params.initializationOptions or {}, { tsserver = { path = lib } })
        end
    end,
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
