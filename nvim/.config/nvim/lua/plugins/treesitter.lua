-- Treesitter. This is the `main` branch rewrite (the old `master` API —
-- nvim-treesitter.configs, ensure_installed, highlight = { enable = true } — no
-- longer exists). Consequences baked into the spec below:
--   * lazy = false is mandatory; the plugin explicitly does not support lazy-loading
--   * parsers are installed by require('nvim-treesitter').install(...)
--   * the plugin enables nothing: vim.treesitter.start() is called per filetype
-- Requires Neovim >= 0.12 and the tree-sitter CLI (Brewfile: brew "tree-sitter").
--
-- Neovim 0.12 bundles only c, lua, markdown, markdown_inline, query, vim, vimdoc.
-- markdown is reinstalled here so parser and queries come from the same revision.
local parsers = {
    'bash', 'diff', 'dockerfile', 'gitcommit', 'go', 'hcl', 'javascript',
    'json', 'just', 'lua', 'markdown', 'markdown_inline', 'nu', 'proto',
    'python', 'query', 'regex', 'rust', 'sql', 'toml', 'tsx', 'typescript',
    'vim', 'vimdoc', 'yaml',
    -- No 'jsonc' parser exists: the plugin maps the jsonc filetype to `json`.
}

return {
    {
        'nvim-treesitter/nvim-treesitter',
        branch = 'main',
        lazy = false,
        build = ':TSUpdate',
        config = function()
            require('nvim-treesitter').install(parsers) -- async; no-op when present

            vim.api.nvim_create_autocmd('FileType', {
                group = vim.api.nvim_create_augroup('dotconfig_treesitter', { clear = true }),
                callback = function(ev)
                    local lang = vim.treesitter.language.get_lang(ev.match)
                    if not lang or not vim.tbl_contains(parsers, lang) then
                        return
                    end
                    -- pcall: on a first run the parser may still be compiling.
                    if not pcall(vim.treesitter.start, ev.buf, lang) then
                        return
                    end
                    vim.wo[0][0].foldmethod = 'expr'
                    vim.wo[0][0].foldexpr = 'v:lua.vim.treesitter.foldexpr()'
                    -- Treesitter indentexpr is left off on purpose: it is flagged
                    -- experimental upstream and conform.nvim reformats on save anyway.
                end,
            })
        end,
    },

    {
        -- Structural *motion*, not selection: mini.ai (lua/plugins/editor.lua)
        -- owns af/if/ac/ic so the two plugins never fight over a keymap.
        -- f/F/t/T and ;/, deliberately stay with flash.nvim.
        'nvim-treesitter/nvim-treesitter-textobjects',
        branch = 'main',
        event = 'VeryLazy',
        dependencies = { 'nvim-treesitter/nvim-treesitter' },
        config = function()
            require('nvim-treesitter-textobjects').setup({ move = { set_jumps = true } })

            local move = require('nvim-treesitter-textobjects.move')
            local swap = require('nvim-treesitter-textobjects.swap')
            local modes = { 'n', 'x', 'o' }

            -- ]m/[m next/previous function, ]]/[[ next/previous class or type.
            local jumps = {
                [']m'] = { move.goto_next_start, '@function.outer', 'Next function start' },
                [']M'] = { move.goto_next_end, '@function.outer', 'Next function end' },
                ['[m'] = { move.goto_previous_start, '@function.outer', 'Previous function start' },
                ['[M'] = { move.goto_previous_end, '@function.outer', 'Previous function end' },
                [']]'] = { move.goto_next_start, '@class.outer', 'Next class start' },
                ['[['] = { move.goto_previous_start, '@class.outer', 'Previous class start' },
            }
            for lhs, spec in pairs(jumps) do
                vim.keymap.set(modes, lhs, function() spec[1](spec[2], 'textobjects') end,
                    { desc = 'TS: ' .. spec[3] })
            end

            -- Reorder arguments/parameters without retyping them.
            vim.keymap.set('n', '<leader>cn', function() swap.swap_next('@parameter.inner') end,
                { desc = 'Swap argument with next' })
            vim.keymap.set('n', '<leader>cp', function() swap.swap_previous('@parameter.inner') end,
                { desc = 'Swap argument with previous' })
        end,
    },
}
