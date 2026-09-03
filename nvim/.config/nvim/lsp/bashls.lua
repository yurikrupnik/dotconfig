-- bash-language-server (node global). Also covers the generated scripts under
-- output/bin/ and config/scripts/*.sh; shellcheck is used when installed.
return {
    cmd = { 'bash-language-server', 'start' },
    filetypes = { 'sh', 'bash' },
    root_markers = { '.git' },
    settings = {
        bashIde = { globPattern = '*@(.sh|.inc|.bash|.command)' },
    },
}
