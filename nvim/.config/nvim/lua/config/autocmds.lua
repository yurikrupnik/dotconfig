-- Autocommands. Each one is grouped so re-sourcing this file cannot duplicate it.
local function group(name)
    return vim.api.nvim_create_augroup('dotconfig_' .. name, { clear = true })
end

-- Flash what was just yanked — the cheapest possible feedback that an operator
-- + motion did what you meant.
vim.api.nvim_create_autocmd('TextYankPost', {
    group = group('highlight_yank'),
    callback = function()
        vim.hl.on_yank({ timeout = 150 })
    end,
})

-- Return to the last cursor position when reopening a file.
vim.api.nvim_create_autocmd('BufReadPost', {
    group = group('last_position'),
    callback = function(ev)
        if vim.bo[ev.buf].filetype == 'gitcommit' then
            return
        end
        local mark = vim.api.nvim_buf_get_mark(ev.buf, '"')
        if mark[1] > 0 and mark[1] <= vim.api.nvim_buf_line_count(ev.buf) then
            pcall(vim.api.nvim_win_set_cursor, 0, mark)
        end
    end,
})

-- Strip trailing whitespace warnings out of the way: 2-space languages used in
-- this repo (yaml/json/ts/lua/nu) override the global 4-space default.
vim.api.nvim_create_autocmd('FileType', {
    group = group('indent'),
    pattern = { 'yaml', 'yml', 'json', 'jsonc', 'javascript', 'typescript',
        'typescriptreact', 'javascriptreact', 'lua', 'nu', 'html', 'css', 'markdown' },
    callback = function()
        vim.bo.shiftwidth = 2
        vim.bo.tabstop = 2
        vim.bo.softtabstop = 2
    end,
})

-- q closes throwaway windows instead of :quit gymnastics.
vim.api.nvim_create_autocmd('FileType', {
    group = group('close_with_q'),
    pattern = { 'help', 'qf', 'man', 'checkhealth', 'lspinfo', 'notify', 'query' },
    callback = function(ev)
        vim.bo[ev.buf].buflisted = false
        vim.keymap.set('n', 'q', '<cmd>close<cr>', { buffer = ev.buf, silent = true })
    end,
})

-- Wrap prose, not code.
vim.api.nvim_create_autocmd('FileType', {
    group = group('prose'),
    pattern = { 'markdown', 'gitcommit', 'text' },
    callback = function()
        vim.wo.wrap = true
        vim.wo.linebreak = true
        vim.bo.textwidth = 80
    end,
})

-- Filetypes this repo uses that Neovim does not detect on its own.
vim.filetype.add({
    extension = { nu = 'nu', k = 'kcl', kcl = 'kcl' },
    filename = { ['Brewfile'] = 'ruby', ['justfile'] = 'just', ['Justfile'] = 'just' },
})
