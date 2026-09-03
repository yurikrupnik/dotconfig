-- Formatting on save, per filetype, using the same binaries CI and the shell use.
-- LSP formatting is the fallback only where no dedicated formatter is declared.
return {
    {
        'stevearc/conform.nvim',
        event = 'BufWritePre',
        cmd = 'ConformInfo',
        keys = {
            {
                '<leader>cf',
                function() require('conform').format({ async = true, lsp_format = 'fallback' }) end,
                mode = { 'n', 'v' },
                desc = 'Format buffer / selection',
            },
            {
                '<leader>uf',
                function()
                    vim.g.disable_autoformat = not vim.g.disable_autoformat
                    vim.notify('Format on save: ' .. (vim.g.disable_autoformat and 'off' or 'on'))
                end,
                desc = 'Toggle format on save',
            },
        },
        opts = {
            formatters_by_ft = {
                lua = { 'stylua' },
                rust = { 'rustfmt' },
                python = { 'ruff_organize_imports', 'ruff_format' },
                sh = { 'shfmt' },
                bash = { 'shfmt' },
                toml = { 'taplo' },
                javascript = { 'prettier' },
                javascriptreact = { 'prettier' },
                typescript = { 'prettier' },
                typescriptreact = { 'prettier' },
                json = { 'prettier' },
                jsonc = { 'prettier' },
                yaml = { 'prettier' },
                markdown = { 'prettier' },
                html = { 'prettier' },
                css = { 'prettier' },
                kcl = { 'kcl_fmt' },
            },
            format_on_save = function(buf)
                if vim.g.disable_autoformat or vim.b[buf].disable_autoformat then
                    return
                end
                -- Never reformat files outside a project (e.g. /tmp, git commit msgs).
                if vim.bo[buf].filetype == 'gitcommit' then
                    return
                end
                return { timeout_ms = 2000, lsp_format = 'fallback' }
            end,
            formatters = {
                shfmt = { prepend_args = { '-i', '4', '-ci' } },
                -- KCL ships its own formatter in the `kcl` binary (brew kcl-lang/tap/kcl).
                kcl_fmt = { command = 'kcl', args = { 'fmt', '$FILENAME' }, stdin = false },
            },
        },
    },
}
