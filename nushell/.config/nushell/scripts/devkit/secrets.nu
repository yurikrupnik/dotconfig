#!/usr/bin/env nu

# Secrets Management
# Fetch and manage secrets using vals (Vault, GCP Secret Manager, etc.)

use common.nu *
use config.nu *

# Sanitize a name segment into a valid env-var fragment (UPPER_SNAKE).
def env-key [s: string]: nothing -> string {
    $s | str uppercase | str replace -a -r "[^A-Z0-9]+" "_" | str trim -c "_"
}

# Recursively flatten a record into [{key, value}]. Nested field names are
# joined with '_'; the caller passes an empty prefix so the parent secret name
# is NOT included. Non-scalar leaves (lists/tables) are compact-JSON encoded.
def flatten-secret [rec: record, prefix: string]: nothing -> list<any> {
    mut out = []
    for col in ($rec | columns) {
        let v = ($rec | get $col)
        let key = (if ($prefix | is-empty) { (env-key $col) } else { $"($prefix)_(env-key $col)" })
        let d = ($v | describe)
        if ($d | str starts-with "record") {
            $out = ($out | append (flatten-secret $v $key))
        } else if ($d | str starts-with "list") or ($d | str starts-with "table") {
            $out = ($out | append {key: $key, value: ($v | to json -r)})
        } else {
            $out = ($out | append {key: $key, value: ($v | into string)})
        }
    }
    $out
}

# Format one KEY=VALUE line. Values with newlines, quotes, or spaces are written
# double-quoted with escaped \n via `to json` (node/python dotenv + compose read
# these); plain scalars stay bare.
def env-line [key: string, value: string]: nothing -> string {
    if ($value | str contains "\n") or ($value | str contains "\"") or ($value | str contains " ") {
        $"($key)=($value | to json)\n"
    } else {
        $"($key)=($value)\n"
    }
}

# vals / vault secret handling. Run a subcommand, or `help devkit secrets <cmd>`.
export def "devkit secrets" [] {
    print "devkit secrets — vals / vault secret handling"
    print ""
    print "  devkit secrets fetch [-o OUT -c CFG]   fetch via vals -> .env"
    print "  devkit secrets vault [-o OUT]          generate .env from Vault"
    print "  devkit secrets load [--env-file F]     load + display .env secrets"
    print "  devkit secrets list [-c CFG]           list configured secrets"
    print "  devkit secrets verify [-c CFG]         dry-run fetch check"
}

# Fetch secrets using vals and generate a .env file.
#
# Refs are resolved via `vals eval` (structured), not `vals env`, so multi-line
# and JSON values survive intact. Secrets that resolve to a JSON object are
# recursively flattened into individual KEY=VALUE lines: nested field names are
# joined with '_' and the parent secret name is NOT used as a prefix (e.g.
# APP_SECRETS.github.token -> GITHUB_TOKEN). Scalars pass through unchanged.
# Duplicate keys collapse to the first occurrence; a warning is emitted only if
# their values actually differ. Use --no-flatten to keep each object as a single
# JSON-valued variable under its own name.
export def "devkit secrets fetch" [
    --output (-o): string = ""  # Output file path (defaults to config secrets.output)
    --config (-c): string = ""  # vals config file (defaults to config secrets.config)
    --no-flatten                # Keep JSON-object secrets as one JSON-valued var
] {
    require-bin "vals"

    let cfg = (resolve-config)
    let output = (if ($output | is-empty) { $cfg.secrets.output } else { $output })
    let config = (if ($config | is-empty) { $cfg.secrets.config } else { $config })

    if not ($config | path exists) {
        error $"Config file not found: ($config)"
        exit 1
    }

    info $"Fetching secrets using vals from ($config)"

    # Resolve every ref into structured data. `vals eval` emits the config with
    # values substituted; `from yaml` gives us a record keyed by env-var name.
    let resolved = (vals eval -f $config | from yaml)

    # Build a flat list of {key, value} pairs.
    mut pairs = []
    for col in ($resolved | columns) {
        let raw = ($resolved | get $col)
        # A ref may resolve to a JSON string; parse it back into structure.
        let val = (try { $raw | from json } catch { $raw })
        let is_obj = (($val | describe) | str starts-with "record")
        if $is_obj and (not $no_flatten) {
            $pairs = ($pairs | append (flatten-secret $val ""))
        } else if $is_obj {
            $pairs = ($pairs | append {key: (env-key $col), value: ($val | to json -r)})
        } else {
            $pairs = ($pairs | append {key: (env-key $col), value: ($raw | into string)})
        }
    }

    # Collapse duplicate keys (first wins); warn only when values truly differ.
    mut seen = {}
    mut ordered = []
    for p in $pairs {
        if ($p.key in ($seen | columns)) {
            if ($seen | get $p.key) != $p.value {
                warn $"  key collision with differing values: ($p.key) \(keeping first\)"
            }
            continue
        }
        $seen = ($seen | insert $p.key $p.value)
        $ordered = ($ordered | append $p)
        info $"  Fetched ($p.key)"
    }

    mut env_content = "# Auto-generated by devkit secrets fetch - DO NOT COMMIT\n"
    $env_content = ($env_content + $"# Generated at: (date now | format date '%Y-%m-%d %H:%M:%S')\n\n")
    for p in $ordered {
        $env_content = ($env_content + (env-line $p.key $p.value))
    }

    $env_content | save --force $output

    success $"($ordered | length) secrets written to ($output)"
    warn $"Never commit the ($output) file to version control!"
}

# Generate .env from Vault
export def "devkit secrets vault" [
    --output (-o): string = ""  # Output file path (defaults to config secrets.output)
] {
    require-bin "vals"

    let cfg = (resolve-config)
    let output = (if ($output | is-empty) { $cfg.secrets.output } else { $output })

    let vault_addr = ($env.VAULT_ADDR? | default "http://localhost:8200")

    if ($env.VAULT_TOKEN? | is-empty) {
        warn "VAULT_TOKEN not set. Attempting to read from ~/.vault-token"
        let token_file = $"($env.HOME)/.vault-token"
        if ($token_file | path exists) {
            $env.VAULT_TOKEN = (open $token_file | str trim)
        } else {
            error "No Vault token found. Run 'vault login' first."
            exit 1
        }
    }

    info $"Fetching secrets from Vault at ($vault_addr)"

    if (".env-vault.template" | path exists) {
        vals eval -f .env-vault.template | save -f $output
        success $"Generated ($output) from Vault"
    } else if (".env-vault.yaml" | path exists) {
        devkit secrets fetch --output $output --config $cfg.secrets.config
    } else {
        error "No .env-vault.template or .env-vault.yaml found"
        exit 1
    }
}

# Load and display secrets from .env file
export def "devkit secrets load" [
    --env-file: string = ""  # Path to .env file (defaults to config secrets.output)
] {
    let cfg = (resolve-config)
    let env_file = (if ($env_file | is-empty) { $cfg.secrets.output } else { $env_file })

    if not ($env_file | path exists) {
        error $"Environment file not found: ($env_file)"
        info "Run 'devkit secrets fetch' to generate it"
        exit 1
    }

    info $"Loading environment variables from ($env_file)"

    let lines = (open $env_file | lines)
    mut count = 0

    for line in $lines {
        if ($line | str starts-with "#") or ($line | str trim | is-empty) {
            continue
        }

        if ($line | str contains "=") {
            let parts = ($line | split row "=")
            if ($parts | length) >= 2 {
                let key = ($parts.0 | str trim)
                info $"  Loaded ($key)"
                $count = $count + 1
            }
        }
    }

    success $"($count) environment variables loaded"
    info "Note: To export, source the file: source .env"
}

# List configured secrets in vals config
export def "devkit secrets list" [
    --config (-c): string = ""  # vals config file (defaults to config secrets.config)
] {
    let cfg = (resolve-config)
    let config = (if ($config | is-empty) { $cfg.secrets.config } else { $config })

    if not ($config | path exists) {
        error $"Config file not found: ($config)"
        exit 1
    }

    let content = (open $config)

    info $"Secrets configured in ($config):\n"

    for entry in ($content | transpose key value) {
        let env_var = $entry.key
        let ref = $entry.value

        if ($ref | str starts-with "ref+vault://") {
            let path = ($ref | str replace "ref+vault://" "")
            print $"  ($env_var) <- vault://($path)"
        } else if ($ref | str starts-with "ref+gcpsecrets://") {
            let path = ($ref | str replace "ref+gcpsecrets://" "")
            print $"  ($env_var) <- gcpsecrets://($path)"
        } else if ($ref | str starts-with "ref+awssecrets://") {
            let path = ($ref | str replace "ref+awssecrets://" "")
            print $"  ($env_var) <- awssecrets://($path)"
        } else {
            print $"  ($env_var) = ($ref)"
        }
    }
}

# Verify secrets can be fetched (dry run)
export def "devkit secrets verify" [
    --config (-c): string = ""  # vals config file (defaults to config secrets.config)
] {
    require-bin "vals"

    let cfg = (resolve-config)
    let config = (if ($config | is-empty) { $cfg.secrets.config } else { $config })

    if not ($config | path exists) {
        error $"Config file not found: ($config)"
        exit 1
    }

    info "Verifying secrets access..."

    let result = (do { vals env -f $config } | complete)

    if $result.exit_code == 0 {
        let count = ($result.stdout | lines | where {|l| $l | str contains "="} | length)
        success $"All ($count) secrets accessible"
    } else {
        error "Failed to fetch secrets:"
        print $result.stderr
        exit 1
    }
}
