-- Navigation and text manipulation — where the actual speed comes from.
return {
    {
        -- Fuzzy everything. fzf-native is a compiled sorter; it needs `make`,
        -- which this machine has via Xcode CLT.
        'nvim-telescope/telescope.nvim',
        cmd = 'Telescope',
        dependencies = {
            'nvim-lua/plenary.nvim',
            { 'nvim-telescope/telescope-fzf-native.nvim', build = 'make' },
        },
        keys = {
            { '<leader><space>', '<cmd>Telescope find_files<cr>', desc = 'Find files' },
            { '<leader>/', '<cmd>Telescope live_grep<cr>', desc = 'Grep workspace' },
            { '<leader>,', '<cmd>Telescope buffers sort_mru=true<cr>', desc = 'Buffers' },
            { '<leader>fr', '<cmd>Telescope oldfiles<cr>', desc = 'Recent files' },
            { '<leader>fg', '<cmd>Telescope git_files<cr>', desc = 'Git files' },
            { '<leader>fh', '<cmd>Telescope help_tags<cr>', desc = 'Help tags' },
            { '<leader>fk', '<cmd>Telescope keymaps<cr>', desc = 'Keymaps' },
            { '<leader>fc', '<cmd>Telescope commands<cr>', desc = 'Commands' },
            { '<leader>sw', '<cmd>Telescope grep_string<cr>', desc = 'Grep word under cursor' },
            { '<leader>sr', '<cmd>Telescope resume<cr>', desc = 'Resume last picker' },
            { '<leader>xx', '<cmd>Telescope diagnostics<cr>', desc = 'Diagnostics' },
            { '<leader>cS', '<cmd>Telescope lsp_dynamic_workspace_symbols<cr>', desc = 'Workspace symbols' },
            { 'grr', '<cmd>Telescope lsp_references<cr>', desc = 'LSP references (picker)' },
        },
        opts = function()
            local actions = require('telescope.actions')
            return {
                defaults = {
                    prompt_prefix = '  ',
                    selection_caret = '▍ ',
                    path_display = { 'truncate' },
                    -- Insert-mode <Esc> closes the picker instead of dropping to
                    -- normal mode inside it — one keystroke, no dead end.
                    mappings = {
                        i = {
                            ['<Esc>'] = actions.close,
                            ['<C-j>'] = actions.move_selection_next,
                            ['<C-k>'] = actions.move_selection_previous,
                            ['<C-q>'] = actions.smart_send_to_qflist + actions.open_qflist,
                        },
                    },
                    file_ignore_patterns = {
                        '^%.git/', 'node_modules/', 'target/', 'dist/', '%.nx/', '%.lock$',
                    },
                },
                pickers = {
                    find_files = { hidden = true },
                    buffers = { ignore_current_buffer = true, theme = 'dropdown' },
                },
            }
        end,
        config = function(_, opts)
            local telescope = require('telescope')
            telescope.setup(opts)
            telescope.load_extension('fzf')
        end,
    },

    {
        -- Directory as an editable buffer: rename/create/delete files with the
        -- same operators you edit text with. Replaces netrw entirely.
        'stevearc/oil.nvim',
        lazy = false, -- must own the netrw hijack before any directory is opened
        dependencies = { 'nvim-tree/nvim-web-devicons' },
        keys = {
            { '-', '<cmd>Oil<cr>', desc = 'Open parent directory' },
            { '<leader>e', function() require('oil').toggle_float() end, desc = 'File explorer (float)' },
        },
        opts = {
            default_file_explorer = true,
            delete_to_trash = true, -- recoverable; this is macOS Trash
            view_options = { show_hidden = true },
            keymaps = { ['q'] = 'actions.close' },
        },
    },

    {
        -- Jump anywhere on screen with 2 keystrokes. This is the single biggest
        -- motion upgrade over hjkl-mashing; it also enhances f/t/;/,.
        'folke/flash.nvim',
        event = 'VeryLazy',
        opts = { modes = { char = { jump_labels = true } } },
        keys = {
            { 's', mode = { 'n', 'x', 'o' }, function() require('flash').jump() end, desc = 'Flash jump' },
            { 'S', mode = { 'n', 'x', 'o' }, function() require('flash').treesitter() end, desc = 'Flash treesitter' },
            { 'r', mode = 'o', function() require('flash').remote() end, desc = 'Remote flash' },
        },
    },

    {
        -- Surroundings as objects: gsa/gsd/gsr add, delete, replace the quotes,
        -- brackets or tags around a text object.
        'echasnovski/mini.surround',
        event = 'VeryLazy',
        opts = {
            mappings = {
                add = 'gsa',      -- visual/normal: add surrounding
                delete = 'gsd',
                replace = 'gsr',
                find = 'gsf',
                find_left = 'gsF',
                highlight = 'gsh',
            },
        },
    },

    {
        -- Smarter a/i text objects. mini.ai owns every *selection* object; the
        -- treesitter plugin owns *motions* (]m, [[) — no keymap overlap.
        -- Built-ins worth knowing: aa/ia = argument, af/if = function,
        -- aq/iq = quote, ab/ib = brackets, at/it = tag, a?/i? = prompt.
        'echasnovski/mini.ai',
        event = 'VeryLazy',
        opts = function()
            local ai = require('mini.ai')
            return {
                n_lines = 500,
                custom_textobjects = {
                    -- Treesitter-backed: af/if = function, ac/ic = class,
                    -- ao/io = any block / conditional / loop.
                    f = ai.gen_spec.treesitter({ a = '@function.outer', i = '@function.inner' }),
                    c = ai.gen_spec.treesitter({ a = '@class.outer', i = '@class.inner' }),
                    o = ai.gen_spec.treesitter({
                        a = { '@block.outer', '@conditional.outer', '@loop.outer' },
                        i = { '@block.inner', '@conditional.inner', '@loop.inner' },
                    }),
                },
            }
        end,
    },

    {
        -- Auto-close pairs, treesitter-aware so it does not fight strings.
        'echasnovski/mini.pairs',
        event = 'InsertEnter',
        opts = {},
    },
}
