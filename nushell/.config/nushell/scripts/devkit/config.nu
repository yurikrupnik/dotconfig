#!/usr/bin/env nu

# devkit configuration layer
#
# Every monorepo-specific fact lives here as a default, overridable per-repo
# via a `devkit.toml` discovered by walking up from $PWD to the filesystem root.
# The engine modules NEVER hardcode paths, namespaces, or endpoints; they read
# them through `resolve-config`. This is what makes the same engine reusable
# across different monorepos.

use common.nu *

# Absolute path to this module's directory (resolved at parse time). Used to
# locate bundled assets (e.g. devkit.toml.example) regardless of the caller's cwd.
const DEVKIT_DIR = path self .

# Built-in defaults. A consumer repo overrides any subset of these in devkit.toml.
# Lists are replaced wholesale by the user value; records are deep-merged.
export const DEFAULTS = {
    # Cluster lifecycle
    cluster: {
        name: "dev"            # default Kind cluster name
        workers: 2             # default worker node count
        db_workers: 1          # tainted db-dedicated workers
        ingress: true          # expose ports 80/443
        # OCI url WITHOUT a tag; kcl resolves the version via --tag (kcl_tag below).
        kcl_package: "oci://docker.io/yurikrupnik/cluster"
        kcl_tag: "0.0.6"
    }

    # Kubernetes namespaces the lifecycle touches
    namespaces: {
        dbs: "dbs"
        monitoring: "monitoring"
        external_secrets: "external-secrets"
    }
    # If non-empty, these app namespaces are created on `up`. If empty, devkit
    # scans `paths.apps_dir` for top-level dirs and uses those.
    app_namespaces: []

    # Repo-relative paths
    paths: {
        apps_dir: "apps"
        compose_file: "manifests/dockers/compose.yaml"
        # `{target}` is substituted with the --target flag (dev/staging/prod).
        overlays: {
            core: "manifests/k8s/overlays/{target}"
            gitops: "manifests/k8s/overlays/{target}"
            observability: "manifests/k8s/overlays/{target}"
        }
        # Default --target for overlay commands
        default_target: "dev"
    }

    # Database (migrations + waits)
    database: {
        port: 5433
        user: "myuser"
        password: "mypassword"
        name: "mydatabase"
        migration_cmd: ["cargo" "run" "-p" "migration" "--" "up"]
        wait_timeout: "120s"
    }

    # Secrets (vals)
    secrets: {
        config: ".vals.yaml"
        output: ".env"
    }

    # External-secrets / GCP
    external_secrets: {
        gcp_credentials: "~/dotconfig/tmp/secret-puller.json"
        secret_name: "gcp-sm-credentials"
    }

    # Flux GitOps bootstrap
    flux: {
        repository: "gitops"
        branch: "main"
        path: "clusters/local"
        personal: true
    }

    # Endpoints printed after `up`. Each row: { label, url }.
    endpoints: [
        { label: "Tilt UI",  url: "http://localhost:10350" }
        { label: "API",      url: "http://localhost:5221/api" }
        { label: "Web",      url: "http://localhost:5173" }
        { label: "Postgres", url: "localhost:5433" }
        { label: "Redis",    url: "localhost:6379" }
    ]

    # Whether `up` should start Tilt at the end
    tilt: { enabled: true }
}

# Find the nearest config file by walking up from $PWD. Returns "" if none.
export def find-config-file [name: string = "devkit.toml"]: nothing -> string {
    let start = ($env.PWD | path expand)
    let parts = ($start | path split)
    # Build the chain of ancestor dirs, deepest first.
    let ancestors = (
        0..(($parts | length) - 1)
        | each {|n| $parts | first ($n + 1) | path join }
        | reverse
    )
    $ancestors
    | where {|d| ($d | path join $name) | path exists }
    | get -o 0
    | default ""
    | if ($in | is-empty) { "" } else { $in | path join $name }
}

# Resolve effective config: DEFAULTS deep-merged with the discovered devkit.toml.
# `--file` forces a specific config path (bypasses discovery).
export def resolve-config [
    --file (-f): string   # explicit config file path
]: nothing -> record {
    let cfg_path = if ($file | is-not-empty) { $file } else { find-config-file }

    if ($cfg_path | is-empty) or (not ($cfg_path | path exists)) {
        return $DEFAULTS
    }

    let user = (open $cfg_path)
    # Lists replace, records deep-merge.
    $DEFAULTS | merge deep --strategy=overwrite $user
}

# Show the effective config (DEFAULTS deep-merged with the discovered devkit.toml).
# Nested records/tables are expanded so every value is visible; use --data for a
# raw record you can pipe (e.g. `devkit config --data | get cluster.name`).
export def "devkit config" [
    --file (-f): string   # explicit config file path
    --path                # only print the resolved config file path
    --data                # output the raw record for piping instead of a table
] {
    if $path {
        let p = if ($file | is-not-empty) { $file } else { find-config-file }
        if ($p | is-empty) {
            info "No devkit.toml found; using built-in defaults"
        } else {
            print $p
        }
        return
    }
    let cfg = (resolve-config --file $file)
    if $data { return $cfg }
    $cfg | table --expand
}

# Scaffold a devkit.toml from the bundled reference into the target directory.
export def "devkit config init" [
    dir?: string        # Target directory (default: git repo root, else $PWD)
    --force             # Overwrite an existing devkit.toml
] {
    let target_dir = if ($dir | is-not-empty) {
        $dir | path expand
    } else {
        let root = (do { git rev-parse --show-toplevel } | complete)
        if $root.exit_code == 0 { $root.stdout | str trim } else { $env.PWD }
    }

    if not ($target_dir | path exists) {
        error $"Target directory does not exist: ($target_dir)"
        exit 1
    }

    let dest = ($target_dir | path join "devkit.toml")
    if ($dest | path exists) and (not $force) {
        error $"($dest) already exists. Use --force to overwrite."
        exit 1
    }

    let example = ($DEVKIT_DIR | path join "devkit.toml.example")
    if not ($example | path exists) {
        error $"Bundled reference not found: ($example)"
        exit 1
    }

    open --raw $example | save --force $dest
    success $"Wrote ($dest)"
    info "Edit only the keys that differ from the built-in defaults."
    info "Inspect the merged result with: devkit config"
}
