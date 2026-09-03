-- LSP wiring, built-in only (Neovim >= 0.11). Per-server settings live in
-- ../../lsp/<name>.lua and are picked up from runtimepath by vim.lsp.enable().
--
-- Server binaries are declared where every other machine package is declared:
--   Brewfile      lua-language-server, ruff, taplo, kcl-language-server, nushell
--   package.json  typescript-language-server, yaml-language-server, pyright,
--                 bash-language-server, vscode-langservers-extracted
--   rustup        rust-analyzer (driven by rustaceanvim, see lua/plugins/rust.lua)
-- Run `u` to install/refresh them. Nothing is downloaded by Neovim itself.

-- name → binary that must exist. A missing binary is skipped silently instead of
-- producing a spawn error on every buffer of that filetype.
local servers = {
    lua_ls = 'lua-language-server',
    ts_ls = 'typescript-language-server',
    jsonls = 'vscode-json-language-server',
    yamlls = 'yaml-language-server',
    bashls = 'bash-language-server',
    pyright = 'pyright-langserver',
    ruff = 'ruff',
    taplo = 'taplo',
    nushell = 'nu',
    kcl = 'kcl-language-server',
}

local enabled, missing = {}, {}
for name, bin in pairs(servers) do
    table.insert(vim.fn.executable(bin) == 1 and enabled or missing, name)
end
vim.lsp.enable(enabled)

-- :LspMissing reports which servers are configured but not installed.
vim.api.nvim_create_user_command('LspMissing', function()
    if #missing == 0 then
        vim.notify('All configured language servers are installed', vim.log.levels.INFO)
    else
        table.sort(missing)
        vim.notify('Missing servers (run `u` to install): ' .. table.concat(missing, ', '),
            vim.log.levels.WARN)
    end
end, { desc = 'List configured-but-missing language servers' })

vim.diagnostic.config({
    severity_sort = true,
    underline = { severity = vim.diagnostic.severity.ERROR },
    virtual_text = { spacing = 2, source = 'if_many', prefix = '●' },
    float = { source = true, header = '', prefix = '' },
    signs = {
        text = {
            [vim.diagnostic.severity.ERROR] = ' ',
            [vim.diagnostic.severity.WARN] = ' ',
            [vim.diagnostic.severity.INFO] = ' ',
            [vim.diagnostic.severity.HINT] = ' ',
        },
    },
})

-- Buffer-local keymaps, attached only where the capability exists.
-- Neovim 0.11 already ships grn (rename), gra (code action), grr (references),
-- gri (implementation), gO (document symbols) and K (hover). The maps below are
-- the IDE-shaped aliases plus the pieces core does not provide.
vim.api.nvim_create_autocmd('LspAttach', {
    group = vim.api.nvim_create_augroup('dotconfig_lsp_attach', { clear = true }),
    callback = function(ev)
        local client = vim.lsp.get_client_by_id(ev.data.client_id)
        local function map(keys, fn, desc, mode)
            vim.keymap.set(mode or 'n', keys, fn, { buffer = ev.buf, desc = 'LSP: ' .. desc })
        end

        map('gd', vim.lsp.buf.definition, 'Goto definition')
        map('gD', vim.lsp.buf.declaration, 'Goto declaration')
        map('gy', vim.lsp.buf.type_definition, 'Goto type definition')
        map('<leader>cr', vim.lsp.buf.rename, 'Rename symbol')
        map('<leader>ca', vim.lsp.buf.code_action, 'Code action', { 'n', 'v' })
        map('<leader>cs', vim.lsp.buf.signature_help, 'Signature help')
        map('<leader>cd', vim.diagnostic.open_float, 'Line diagnostics')
        map('<leader>ci', function() vim.cmd.checkhealth('vim.lsp') end, 'LSP health')

        -- Inlay hints: real type information inline. Toggle, because they add noise
        -- while reading unfamiliar code.
        if client and client:supports_method('textDocument/inlayHint') then
            vim.lsp.inlay_hint.enable(true, { bufnr = ev.buf })
            map('<leader>ch', function()
                vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled({ bufnr = ev.buf }),
                    { bufnr = ev.buf })
            end, 'Toggle inlay hints')
        end

        -- Highlight other references to the symbol under the cursor.
        if client and client:supports_method('textDocument/documentHighlight') then
            local hl = vim.api.nvim_create_augroup('dotconfig_lsp_highlight_' .. ev.buf,
                { clear = true })
            vim.api.nvim_create_autocmd({ 'CursorHold', 'CursorHoldI' }, {
                group = hl,
                buffer = ev.buf,
                callback = vim.lsp.buf.document_highlight,
            })
            vim.api.nvim_create_autocmd({ 'CursorMoved', 'CursorMovedI' }, {
                group = hl,
                buffer = ev.buf,
                callback = vim.lsp.buf.clear_references,
            })
        end
    end,
})
