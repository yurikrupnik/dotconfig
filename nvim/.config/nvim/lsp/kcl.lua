-- kcl-language-server (brew kcl-lang/tap/kcl) — KCL configuration language.
return {
    cmd = { 'kcl-language-server' },
    filetypes = { 'kcl' },
    root_markers = { 'kcl.mod', '.git' },
}
