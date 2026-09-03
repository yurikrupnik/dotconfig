-- lua-language-server (brew). Configured for editing this very config: it needs
-- to know about the `vim` global and the Neovim runtime API.
return {
    cmd = { 'lua-language-server' },
    filetypes = { 'lua' },
    root_markers = { '.luarc.json', '.luarc.jsonc', 'stylua.toml', '.git' },
    settings = {
        Lua = {
            runtime = { version = 'LuaJIT' },
            workspace = {
                checkThirdParty = false,
                library = vim.api.nvim_get_runtime_file('', true),
            },
            diagnostics = { globals = { 'vim' } },
            format = { enable = false }, -- stylua owns formatting (see plugins/format.lua)
            telemetry = { enable = false },
            hint = { enable = true },
        },
    },
}
