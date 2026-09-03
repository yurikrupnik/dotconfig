-- Rust is this machine's primary language (config.toml: PROJECT = "rust").
-- rustaceanvim owns the rust-analyzer client entirely — do NOT also enable a
-- rust_analyzer entry via vim.lsp.enable(), that would start two servers.
-- The binary comes from rustup (`rustup component add rust-analyzer`).
return {
    {
        'mrcjkb/rustaceanvim',
        version = '^6',
        lazy = false, -- the plugin is its own ft-plugin; deferring it breaks :RustLsp
        init = function()
            vim.g.rustaceanvim = {
                server = {
                    on_attach = function(_, buf)
                        local function map(keys, cmd, desc)
                            vim.keymap.set('n', keys, cmd, { buffer = buf, desc = 'Rust: ' .. desc })
                        end
                        -- Rust-specific extras on top of the generic LSP maps.
                        map('<leader>rr', function() vim.cmd.RustLsp('runnables') end, 'Runnables')
                        map('<leader>rt', function() vim.cmd.RustLsp('testables') end, 'Testables')
                        map('<leader>rd', function() vim.cmd.RustLsp('debuggables') end, 'Debuggables')
                        map('<leader>rm', function() vim.cmd.RustLsp('expandMacro') end, 'Expand macro')
                        map('<leader>re', function() vim.cmd.RustLsp('explainError') end, 'Explain error')
                        map('<leader>rc', function() vim.cmd.RustLsp('openCargo') end, 'Open Cargo.toml')
                        map('<leader>rD', function() vim.cmd.RustLsp('renderDiagnostic') end, 'Render diagnostic')
                        -- K twice = hover actions (jump into types from the popup).
                        map('K', function() vim.cmd.RustLsp({ 'hover', 'actions' }) end, 'Hover actions')
                    end,
                    default_settings = {
                        ['rust-analyzer'] = {
                            -- clippy on save: the lints that actually catch bugs.
                            checkOnSave = true,
                            check = { command = 'clippy', extraArgs = { '--all-targets' } },
                            cargo = { features = 'all', buildScripts = { enable = true } },
                            procMacro = { enable = true },
                            inlayHints = {
                                lifetimeElisionHints = { enable = 'skip_trivial' },
                                closureReturnTypeHints = { enable = 'with_block' },
                            },
                            files = { excludeDirs = { '.direnv', 'node_modules', 'target' } },
                        },
                    },
                },
                tools = { float_win_config = { border = 'rounded' } },
            }
        end,
    },
}
