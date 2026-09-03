-- lazy.nvim bootstrap. The plugin manager itself is cloned into stdpath('data'),
-- never into this repo — the repo only holds specs plus lazy-lock.json.
local lazypath = vim.fn.stdpath('data') .. '/lazy/lazy.nvim'

-- Where lazy-lock.json is written. stdpath('config') is ~/.config/nvim, whose
-- files are stow symlinks into this repo; a new file created there would be a
-- real file *outside* git. Resolving init.lua's symlink gives the repo copy, so
-- `:Lazy sync` updates a lockfile that is actually committed. Falls back to
-- stdpath('config') when the config is a plain directory (no stow).
local lockfile = vim.fs.joinpath(
    vim.fs.dirname(vim.fn.resolve(vim.fn.stdpath('config') .. '/init.lua')),
    'lazy-lock.json'
)

if not vim.uv.fs_stat(lazypath) then
    local out = vim.fn.system({
        'git', 'clone', '--filter=blob:none', '--branch=stable',
        'https://github.com/folke/lazy.nvim.git', lazypath,
    })
    if vim.v.shell_error ~= 0 then
        error('Failed to clone lazy.nvim:\n' .. out)
    end
end

vim.opt.rtp:prepend(lazypath)

require('lazy').setup({
    spec = { { import = 'plugins' } },
    install = { colorscheme = { 'tokyonight' } },
    checker = { enabled = false }, -- `u` refreshes the machine; :Lazy sync is explicit
    change_detection = { notify = false },
    rocks = { enabled = false }, -- no plugin here needs luarocks; skips the hererocks setup
    performance = {
        rtp = {
            -- Disable stock plugins replaced by better ones or unused here.
            -- `tutor` stays enabled: the learning plan runs :Tutor (shell: `vt`).
            disabled_plugins = { 'gzip', 'tarPlugin', 'tohtml', 'zipPlugin' },
        },
    },
    lockfile = lockfile,
})
