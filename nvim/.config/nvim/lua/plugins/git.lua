-- Git. lazygit already lives in the shell (`lg`), so Neovim only needs the
-- in-buffer half: signs, hunk staging, blame, and diffs against the index.
return {
    {
        'lewis6991/gitsigns.nvim',
        event = { 'BufReadPre', 'BufNewFile' },
        opts = {
            signs = {
                add = { text = '▎' },
                change = { text = '▎' },
                delete = { text = '' },
                topdelete = { text = '' },
                changedelete = { text = '▎' },
                untracked = { text = '▎' },
            },
            on_attach = function(buf)
                local gs = require('gitsigns')
                local function map(keys, fn, desc, mode)
                    vim.keymap.set(mode or 'n', keys, fn, { buffer = buf, desc = 'Git: ' .. desc })
                end

                -- ]h / [h are hunk motions; nav_hunk handles diff windows too.
                map(']h', function() gs.nav_hunk('next') end, 'Next hunk')
                map('[h', function() gs.nav_hunk('prev') end, 'Previous hunk')

                map('<leader>gs', gs.stage_hunk, 'Stage hunk')
                map('<leader>gr', gs.reset_hunk, 'Reset hunk')
                map('<leader>gs', function()
                    gs.stage_hunk({ vim.fn.line('.'), vim.fn.line('v') })
                end, 'Stage selection', 'v')
                map('<leader>gS', gs.stage_buffer, 'Stage buffer')
                map('<leader>gp', gs.preview_hunk, 'Preview hunk')
                map('<leader>gb', function() gs.blame_line({ full = true }) end, 'Blame line')
                map('<leader>gB', gs.blame, 'Blame buffer')
                map('<leader>gd', gs.diffthis, 'Diff against index')
                map('<leader>gD', function() gs.diffthis('~') end, 'Diff against last commit')
                -- ih is a hunk text object: `vih` selects it, `dih` reverts it.
                map('ih', gs.select_hunk, 'Select hunk', { 'o', 'x' })
            end,
        },
    },
}
