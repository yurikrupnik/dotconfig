-- Editor settings. Only deviations from Neovim defaults are listed; 0.11 already
-- sets sensible values for incsearch, hlsearch, autoindent, wildmenu, ttimeout…
local o = vim.opt

-- Files & undo: no swap/backup (git + jj are the safety net), but persist undo
-- across sessions so `u` still works tomorrow.
o.swapfile = false
o.backup = false
o.undofile = true
o.updatetime = 200 -- ms before CursorHold → gitsigns/diagnostics refresh

-- Indentation: matches .editorconfig at the repo root (4 spaces, hard tabs off
-- for most languages). Per-filetype overrides live in lua/config/autocmds.lua.
o.expandtab = true
o.shiftwidth = 4
o.tabstop = 4
o.softtabstop = 4
o.smartindent = true

-- Search: case-insensitive until you type a capital.
o.ignorecase = true
o.smartcase = true

-- UI
o.number = true
o.relativenumber = true -- makes 5j / d3k countable at a glance — key for learning
o.signcolumn = 'yes' -- reserve the gutter so diagnostics don't shift text
o.cursorline = true
o.scrolloff = 8 -- keep context around the cursor
o.sidescrolloff = 8
o.wrap = false
o.splitright = true
o.splitbelow = true
o.termguicolors = true
o.showmode = false -- lualine already shows the mode
o.laststatus = 3 -- one global statusline, not one per split
o.winborder = 'rounded' -- 0.11+: rounded borders for all floats (hover, lazy, …)
o.list = true
o.listchars = { tab = '» ', trail = '·', nbsp = '␣' }
o.confirm = true -- prompt instead of failing on :q with unsaved changes
o.inccommand = 'split' -- live preview for :s///

-- Completion: menuone+noselect keeps you in control of the first entry.
o.completeopt = { 'menu', 'menuone', 'noselect' }
o.pumheight = 12

-- Clipboard: share with macOS. Scheduled so it doesn't slow startup.
vim.schedule(function()
    o.clipboard = 'unnamedplus'
end)

-- Diff/fold: treesitter-driven folds, but everything open on entry.
o.foldmethod = 'expr'
o.foldexpr = 'v:lua.vim.treesitter.foldexpr()'
o.foldlevel = 99
o.foldtext = ''

-- grep: this machine has ripgrep (Brewfile), so :grep uses it.
o.grepprg = 'rg --vimgrep --smart-case'
o.grepformat = '%f:%l:%c:%m'
