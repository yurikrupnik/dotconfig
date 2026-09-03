-- Completion. blink.cmp is used instead of nvim-cmp: one plugin, a Rust fuzzy
-- matcher, and — on Neovim 0.11+ — it registers its own LSP capabilities with
-- vim.lsp.config('*') automatically, so lua/config/lsp.lua stays capability-free.
--
-- Keymap preset 'default' (super-tab is a habit that fights snippets):
--   <C-space> open menu / docs      <C-n>/<C-p> or <Up>/<Down> cycle
--   <C-y>     accept                <C-e> cancel
--   <Tab>/<S-Tab> jump between snippet placeholders
--   <C-k>     toggle signature help
return {
    {
        'saghen/blink.cmp',
        version = '1.*', -- release tag ships a prebuilt matcher; no cargo build
        event = 'InsertEnter',
        dependencies = { 'rafamadriz/friendly-snippets' },
        opts = {
            keymap = { preset = 'default' },
            appearance = { nerd_font_variant = 'mono' },
            completion = {
                documentation = { auto_show = true, auto_show_delay_ms = 250 },
                ghost_text = { enabled = false }, -- distracting while learning motions
                menu = { draw = { treesitter = { 'lsp' } } },
            },
            sources = { default = { 'lsp', 'path', 'snippets', 'buffer' } },
            signature = { enabled = true },
            fuzzy = { implementation = 'prefer_rust_with_warning' },
        },
        opts_extend = { 'sources.default' },
    },
}
