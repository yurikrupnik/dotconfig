-- Look and discoverability. which-key is the load-bearing one while learning:
-- press a prefix, see every continuation instead of grepping this repo.
return {
    {
        'folke/tokyonight.nvim',
        lazy = false,
        priority = 1000, -- colorscheme must load before everything else
        opts = { style = 'night', styles = { comments = { italic = true } } },
        config = function(_, opts)
            require('tokyonight').setup(opts)
            vim.cmd.colorscheme('tokyonight')
        end,
    },

    {
        'folke/which-key.nvim',
        event = 'VeryLazy',
        opts = {
            preset = 'helix',
            delay = 300, -- ms; long enough not to flash, short enough to teach
            spec = {
                { '<leader>b', group = 'buffer' },
                { '<leader>c', group = 'code / lsp' },
                { '<leader>f', group = 'find' },
                { '<leader>g', group = 'git' },
                { '<leader>q', group = 'quit' },
                { '<leader>s', group = 'search' },
                { '<leader>x', group = 'diagnostics / lists' },
            },
        },
        keys = {
            {
                '<leader>?',
                function() require('which-key').show({ global = false }) end,
                desc = 'Buffer-local keymaps',
            },
        },
    },

    {
        'nvim-lualine/lualine.nvim',
        event = 'VeryLazy',
        opts = {
            options = {
                theme = 'tokyonight',
                globalstatus = true,
                section_separators = '',
                component_separators = '|',
            },
            sections = {
                lualine_c = {
                    { 'filename', path = 1 }, -- relative path, not just basename
                    'diagnostics',
                },
                lualine_x = { 'filetype' },
                lualine_y = { 'progress' },
                lualine_z = { 'location' },
            },
        },
    },
}
