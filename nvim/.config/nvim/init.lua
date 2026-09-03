-- Neovim entry point — hand-written stow package (see AGENTS.md: not generated).
-- Layout:
--   lua/config/*   core settings, split by concern
--   lua/plugins/*  one file per concern; every spec is lazy.nvim's format
--   lsp/*.lua      one file per language server, read natively by vim.lsp.enable()
--   docs/          the learning plan (<leader>L opens it)
--
-- Requires Neovim >= 0.12 (nvim-treesitter's `main` branch enforces it). LSP is
-- configured with the built-in vim.lsp.config API, so there is no nvim-lspconfig
-- and no mason: servers are real binaries declared in ../brew/Brewfile and
-- ../node/package.json and installed by `u`.

if vim.fn.has('nvim-0.12') == 0 then
    error('This config requires Neovim >= 0.12 — run: brew upgrade neovim')
end

-- Leader must be set before lazy.nvim loads, or plugin mappings bind to the old one.
vim.g.mapleader = ' '
vim.g.maplocalleader = '\\'

-- Trainer mode: arrow keys and the mouse refuse to move the cursor and tell you
-- the hjkl equivalent instead. This is the forcing function for the 4-week plan
-- in docs/learning-plan.md. Flip to false once motions are automatic.
vim.g.trainer = true

require('config.options')
require('config.lazy') -- bootstraps lazy.nvim and imports lua/plugins/
require('config.keymaps')
require('config.autocmds')
require('config.lsp')
