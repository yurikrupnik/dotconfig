-- Keymaps. Two rules keep this list learnable:
--   1. <leader> is a mnemonic namespace (f=find, g=git, c=code, x=diagnostics).
--   2. Nothing here overrides a core motion — muscle memory must transfer to
--      Zed/IntelliJ vim mode unchanged.
-- Plugin-owned maps live next to their spec in lua/plugins/.
local map = vim.keymap.set

-- Escape hatches
map('n', '<Esc>', '<cmd>nohlsearch<cr>', { desc = 'Clear search highlight' })
map('t', '<Esc><Esc>', '<C-\\><C-n>', { desc = 'Leave terminal mode' })

-- Window navigation. Same chords work in tmux/zellij, so they are worth the habit.
map('n', '<C-h>', '<C-w>h', { desc = 'Window left' })
map('n', '<C-j>', '<C-w>j', { desc = 'Window down' })
map('n', '<C-k>', '<C-w>k', { desc = 'Window up' })
map('n', '<C-l>', '<C-w>l', { desc = 'Window right' })

-- Buffers
map('n', '<S-h>', '<cmd>bprevious<cr>', { desc = 'Previous buffer' })
map('n', '<S-l>', '<cmd>bnext<cr>', { desc = 'Next buffer' })
map('n', '<leader>bd', '<cmd>bdelete<cr>', { desc = 'Delete buffer' })

-- Save / quit — the two things IDE users reach for by reflex.
map({ 'n', 'i', 'v' }, '<C-s>', '<cmd>write<cr><esc>', { desc = 'Save file' })
map('n', '<leader>qq', '<cmd>qa<cr>', { desc = 'Quit all' })

-- Move and re-indent selections: the payoff move for visual mode.
map('v', 'J', ":m '>+1<cr>gv=gv", { desc = 'Move selection down' })
map('v', 'K', ":m '<-2<cr>gv=gv", { desc = 'Move selection up' })
map('v', '<', '<gv', { desc = 'Outdent, keep selection' })
map('v', '>', '>gv', { desc = 'Indent, keep selection' })

-- Keep the cursor centered on big jumps and search hops.
map('n', '<C-d>', '<C-d>zz')
map('n', '<C-u>', '<C-u>zz')
map('n', 'n', 'nzzzv')
map('n', 'N', 'Nzzzv')

-- Paste over a selection without clobbering the unnamed register.
map('x', 'p', 'P', { desc = 'Paste without yanking replaced text' })

-- Quickfix: LSP references / :grep results land here.
map('n', ']q', '<cmd>cnext<cr>', { desc = 'Next quickfix item' })
map('n', '[q', '<cmd>cprevious<cr>', { desc = 'Previous quickfix item' })
map('n', '<leader>xq', '<cmd>copen<cr>', { desc = 'Quickfix list' })

-- Learning plan. Opens docs/learning-plan.md from this config, read-only-ish.
map('n', '<leader>L', function()
    vim.cmd.edit(vim.fn.stdpath('config') .. '/docs/learning-plan.md')
end, { desc = 'Learning plan' })

-- Trainer mode: arrows and mouse clicks refuse to move and teach the hjkl form.
-- Set vim.g.trainer = false in init.lua when the habit sticks.
if vim.g.trainer then
    local hints = {
        ['<Up>'] = 'k', ['<Down>'] = 'j', ['<Left>'] = 'h', ['<Right>'] = 'l',
        ['<PageUp>'] = '<C-u>', ['<PageDown>'] = '<C-d>',
        ['<Home>'] = '0 (or ^)', ['<End>'] = '$',
    }
    for key, hint in pairs(hints) do
        map({ 'n', 'v', 'i' }, key, function()
            vim.notify('trainer: use ' .. hint, vim.log.levels.WARN, { title = 'hjkl' })
        end, { desc = 'Trainer: prefer ' .. hint })
    end
    vim.opt.mouse = '' -- no click-to-position, no scroll-wheel scrolling
end
