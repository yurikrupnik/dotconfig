-- yaml-language-server (node global). This machine writes a lot of Kubernetes,
-- so the k8s schema is wired to the usual manifest locations, and the Kubernetes
-- CRD-heavy tools (Argo, Flux, Crossplane) get their schemas from schemastore.
return {
    cmd = { 'yaml-language-server', '--stdio' },
    filetypes = { 'yaml' }, -- the yaml.* pseudo-filetypes are not detected by Neovim
    root_markers = { '.git' },
    settings = {
        yaml = {
            keyOrdering = false,
            format = { enable = false }, -- prettier formats YAML
            validate = true,
            schemaStore = { enable = true, url = 'https://www.schemastore.org/api/json/catalog.json' },
            schemas = {
                kubernetes = { 'k8s/**/*.yaml', 'manifests/**/*.yaml', '**/*.k8s.yaml' },
                ['https://json.schemastore.org/github-workflow.json'] = '.github/workflows/*.{yml,yaml}',
                ['https://json.schemastore.org/github-action.json'] = '.github/action.{yml,yaml}',
                ['https://raw.githubusercontent.com/compose-spec/compose-spec/master/schema/compose-spec.json'] =
                    '{docker-compose,compose}*.{yml,yaml}',
            },
        },
    },
}
