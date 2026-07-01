#!/usr/bin/env nu
# Update project deps under $PWD to latest stable versions.
#
# Modes:
#   (default)    safe   — OSV-Scanner pre/post, --ignore-scripts, cooldown gate for npm,
#                          post-update build/lint/test via just or nx
#   --fast              — skip all checks/tests, just update --latest. Use knowingly.
#   --paranoid          — safe + cargo-vet + Socket if installed
#
# Granular toggles override the mode preset:
#   --cooldown <days>   release-age gate for npm (default 7; 0 = off)
#   --no-cooldown       same as --cooldown 0
#   --no-scan           skip OSV-Scanner
#   --no-tests          skip post-update build/lint/test
#   --allow-scripts     allow npm/bun/pnpm lifecycle scripts during install
#   --with-vet          add cargo-vet to cargo flow
#   --with-socket       use Socket CLI if installed
#   --with-sfw          wrap install-time commands with `sfw` (Socket Firewall)
#                          when available — proxies cargo/bun/pnpm/uv fetches
#                          and blocks confirmed-malicious packages. Auto-on in
#                          --paranoid mode.
#   --only <list>       comma-separated ecosystems to run: cargo,node,uv.
#                          default: all detected. e.g. --only cargo,uv skips
#                          the node step entirely.
#
# Detects Cargo.toml, package.json, pyproject.toml across the repo (walks Nx libs/apps).
# Emits a machine-readable diagnosis block on failure for AI consumption.

use std log

const PRUNE_GLOBS = [
    "**/node_modules/**" "**/target/**" "**/dist/**" "**/build/**"
    "**/.git/**" "**/.nx/**" "**/.next/**" "**/.turbo/**"
    "**/.venv/**" "**/venv/**" "**/__pycache__/**"
]

def has-cmd [name: string]: nothing -> bool {
    (which $name | length) > 0
}

def find-manifests [name: string]: nothing -> list<string> {
    glob $"**/($name)" --exclude $PRUNE_GLOBS
    | each { |p| $p | into string }
}

# Cargo.toml is a workspace root iff it has a [workspace] table. Members share
# the root's Cargo.lock, and `cargo update`/`cargo upgrade` at the root are
# workspace-aware — they walk every member's manifest. Iterating members is
# pure overhead.
def is-cargo-workspace-root [manifest: string]: nothing -> bool {
    try { "workspace" in (open $manifest) } catch { false }
}

# package.json is a workspace root iff it has a "workspaces" key (npm/yarn/bun
# workspaces), or a pnpm-workspace.yaml sits next to it (pnpm workspaces).
def is-node-workspace-root [manifest: string]: nothing -> bool {
    let dir = $manifest | path dirname
    if (($dir | path join "pnpm-workspace.yaml") | path exists) { return true }
    try { "workspaces" in (open $manifest) } catch { false }
}

# Drop manifests that are workspace members of another manifest in the list.
# Keep workspace roots themselves, and standalone manifests (no ancestor root
# found in the list). Nested workspaces — a root inside another workspace —
# are kept since cargo/pnpm treat them as separate workspaces.
def dedupe-workspace-members [files: list<string>, is_root_fn: closure]: nothing -> list<string> {
    let roots = $files | where { |f| do $is_root_fn $f }
    let root_dirs = $roots | each { |f| $f | path dirname }
    $files | where { |f|
        let f_dir = $f | path dirname
        let f_is_root = ($f in $roots)
        let f_is_member = $root_dirs | any { |r|
            $r != $f_dir and ($f_dir | str starts-with ($r + "/"))
        }
        $f_is_root or (not $f_is_member)
    }
}

def section [msg: string] {
    print $"\n(ansi green_bold)==>(ansi reset) ($msg)"
}

def warn [msg: string] {
    print --stderr $"(ansi yellow)!!(ansi reset) ($msg)"
}

def err-out [msg: string] {
    print --stderr $"(ansi red)xx(ansi reset) ($msg)"
}

# Run an external command, return true on success, false on failure.
def try-run [label: string, block: closure]: nothing -> bool {
    try {
        do $block
        true
    } catch { |e|
        warn $"($label) failed: ($e.msg)"
        false
    }
}

# Run an install-time package-manager command, optionally proxied through
# `sfw` when cfg.sfw is on and sfw is on PATH. `cmd` is a list whose first
# element is the program, e.g. ["cargo" "update"].
def run-pm [label: string, cmd: list<string>, cfg: record]: nothing -> bool {
    let wrapped = if $cfg.sfw and (has-cmd "sfw") {
        ["sfw"] ++ $cmd
    } else {
        $cmd
    }
    try-run $label { ^($wrapped.0) ...($wrapped | skip 1) }
}

# ---------- OSV-Scanner ----------

def run-osv [phase: string]: nothing -> record {
    if not (has-cmd "osv-scanner") {
        warn "osv-scanner not installed; skipping scan  (brew install osv-scanner)"
        return {ok: true, vulns: 0, skipped: true}
    }
    section $"OSV-Scanner — ($phase)"
    let raw = try {
        ^osv-scanner --format=json --recursive . | complete
    } catch {
        return {ok: false, vulns: -1, skipped: false}
    }
    if $raw.exit_code != 0 and ($raw.stdout | is-empty) {
        warn $"osv-scanner errored: ($raw.stderr)"
        return {ok: false, vulns: -1, skipped: false}
    }
    let parsed = try { $raw.stdout | from json } catch { {results: []} }
    let count = (
        $parsed.results? | default []
        | each { |r| ($r.packages? | default [] | length) }
        | math sum
    )
    if $count > 0 {
        warn $"($phase): ($count) vulnerable package group(s) — see details:"
        ^osv-scanner --recursive .
        return {ok: false, vulns: $count, skipped: false}
    }
    log info $"($phase): 0 vulnerabilities"
    {ok: true, vulns: 0, skipped: false}
}

# ---------- Cargo ----------

def update-cargo [files: list<string>, cfg: record]: nothing -> list<record> {
    let has_upgrade = (has-cmd "cargo-upgrade")
    if not $has_upgrade and $cfg.mode != "fast" {
        warn "cargo-edit not installed; cross-major bumps skipped  (cargo install cargo-edit)"
    }
    $files | each { |f|
        cd ($f | path dirname)
        section $"Rust — ($f)"
        let bump_ok = if $has_upgrade and $cfg.mode != "fast" {
            try-run "cargo upgrade" { ^cargo upgrade --incompatible }
        } else { true }
        let update_ok = run-pm "cargo update" ["cargo" "update"] $cfg
        let vet_ok = if $cfg.vet and (has-cmd "cargo-vet") {
            try-run "cargo vet" { ^cargo vet }
        } else { true }
        {file: $f, ecosystem: "cargo", ok: ($bump_ok and $update_ok and $vet_ok)}
    }
}

# ---------- Node (bun / pnpm / npm) ----------

def detect-node-pm []: nothing -> string {
    mut pm = ""
    if ("bun.lock" | path exists) or ("bun.lockb" | path exists) { $pm = "bun" }
    if $pm == "" and ("pnpm-lock.yaml" | path exists) { $pm = "pnpm" }
    if $pm == "" and (("package-lock.json" | path exists) or ("yarn.lock" | path exists)) { $pm = "npm" }
    if $pm == "" and (has-cmd "bun") { $pm = "bun" }
    if $pm == "" and (has-cmd "npm") { $pm = "npm" }
    $pm
}

def update-node [files: list<string>, cfg: record]: nothing -> list<record> {
    let has_ncu = (has-cmd "ncu")
    $files | each { |f|
        cd ($f | path dirname)
        let pm = detect-node-pm
        if ($pm | is-empty) {
            err-out $"no node package manager available for ($f)"
            return {file: $f, ecosystem: "node", ok: false}
        }
        section $"Node ($pm) — ($f)"

        # Step 1: bump package.json
        let bump_ok = if $cfg.mode == "fast" {
            true  # let the install step do whatever within semver
        } else if $has_ncu {
            try-run "ncu" {
                if $cfg.cooldown > 0 {
                    ^ncu --cooldown $cfg.cooldown --target latest -u
                } else {
                    ^ncu --target latest -u
                }
            }
        } else {
            warn "ncu not installed; cross-major bumps skipped  (bun add -g npm-check-updates)"
            true
        }

        # Step 2: install
        let install_cmd = match $pm {
            "bun"  => (if $cfg.ignore_scripts { ["bun" "install" "--ignore-scripts"] } else { ["bun" "install"] }),
            "pnpm" => (if $cfg.ignore_scripts { ["pnpm" "install" "--ignore-scripts"] } else { ["pnpm" "install"] }),
            "npm"  => (if $cfg.ignore_scripts { ["npm" "install" "--ignore-scripts"] } else { ["npm" "install"] }),
            _ => []
        }
        let install_ok = run-pm ($install_cmd | str join " ") $install_cmd $cfg

        # Step 3: fast-mode fallback bump (if ncu wasn't used)
        let extra_ok = if $cfg.mode == "fast" and $pm == "bun" {
            run-pm "bun update --latest" ["bun" "update" "--latest"] $cfg
        } else { true }

        {file: $f, ecosystem: $"node:($pm)", ok: ($bump_ok and $install_ok and $extra_ok)}
    }
}

# ---------- Python (uv) ----------

def update-uv [files: list<string>, cfg: record]: nothing -> list<record> {
    $files | each { |f|
        let dir = ($f | path dirname)
        if not (($dir | path join "uv.lock") | path exists) {
            return null
        }
        cd $dir
        section $"uv — ($f)"
        let lock_ok = run-pm "uv lock --upgrade" ["uv" "lock" "--upgrade"] $cfg
        let sync_ok = run-pm "uv sync" ["uv" "sync"] $cfg
        let bump_ok = if (has-cmd "uv-bump") {
            try-run "uv-bump" { ^uv-bump }
        } else {
            warn "uv-bump not installed; pyproject.toml minimums NOT updated  (uv tool install uv-bump)"
            true
        }
        {file: $f, ecosystem: "uv", ok: ($lock_ok and $sync_ok and $bump_ok)}
    } | where { |x| $x != null }
}

# ---------- Post-update checks ----------

def detect-task-runner []: nothing -> record {
    if ("justfile" | path exists) {
        let summary = try {
            ^just --summary | str trim | split row " "
        } catch { [] }
        let recipes = ($summary | where { |r| $r in ["build" "lint" "test" "check"] })
        if not ($recipes | is-empty) {
            return {kind: "just", recipes: $recipes}
        }
    }
    if ("nx.json" | path exists) {
        return {kind: "nx", recipes: []}
    }
    {kind: "none", recipes: []}
}

def run-checks []: nothing -> record {
    let runner = (detect-task-runner)
    if $runner.kind == "none" {
        warn "no justfile or nx.json — skipping post-update checks"
        return {ok: true, runner: "none", failed: []}
    }
    section $"Post-update checks via ($runner.kind)"
    let results = match $runner.kind {
        "just" => ($runner.recipes | each { |r|
            section $"just ($r)"
            {name: $"just ($r)", ok: (try-run $"just ($r)" { ^just $r })}
        }),
        "nx" => [{
            name: "bun nx affected -t lint build test"
            ok: (try-run "nx affected" { ^bun nx affected -t lint build test --parallel })
        }],
        _ => []
    }
    let failed = ($results | where { |r| not $r.ok } | get name)
    {ok: ($failed | is-empty), runner: $runner.kind, failed: $failed}
}

# ---------- Diagnosis block (for AI) ----------

def emit-diagnosis [report: record] {
    section "DIAGNOSIS (machine-readable)"
    print "BEGIN-UPKG-DIAGNOSIS"
    print ($report | to json)
    print "END-UPKG-DIAGNOSIS"
}

# ---------- main ----------

def --env main [
    --fast            # skip all checks and tests
    --paranoid        # safe + cargo-vet + Socket
    --cooldown: int = 7
    --no-cooldown
    --no-scan
    --no-tests
    --allow-scripts
    --with-vet
    --with-socket
    --with-sfw
    --only: string = ""  # comma-separated ecosystems: cargo,node,uv. Empty = all.
] {
    let mode = if $fast { "fast" } else if $paranoid { "paranoid" } else { "safe" }
    let cfg = {
        mode: $mode
        cooldown: (if $no_cooldown or $fast { 0 } else { $cooldown })
        scan: (not $no_scan and not $fast)
        tests: (not $no_tests and not $fast)
        ignore_scripts: (not $allow_scripts)
        vet: ($with_vet or $paranoid)
        socket: ($with_socket or $paranoid)
        sfw: ($with_sfw or $paranoid)
    }
    if $cfg.sfw and not (has-cmd "sfw") {
        warn "sfw not installed; install-time firewall disabled  (bun add -g sfw)"
    }

    let valid_ecosystems = ["cargo" "node" "uv"]
    let allowed = if ($only | str trim | is-empty) {
        $valid_ecosystems
    } else {
        let parsed = $only | split row "," | each { |s| $s | str trim } | where { |s| not ($s | is-empty) }
        let invalid = $parsed | where { |x| not ($x in $valid_ecosystems) }
        if not ($invalid | is-empty) {
            err-out $"--only: unknown ecosystem: ($invalid | str join ', '). Valid: cargo, node, uv"
            exit 1
        }
        $parsed
    }
    log info $"upkg mode=($cfg.mode) cooldown=($cfg.cooldown)d scan=($cfg.scan) tests=($cfg.tests) ignore_scripts=($cfg.ignore_scripts) vet=($cfg.vet) sfw=($cfg.sfw) only=($allowed | str join ',')"

    let cargo_files = if ("cargo" in $allowed) {
        let all = (find-manifests "Cargo.toml")
        let kept = (dedupe-workspace-members $all { |f| is-cargo-workspace-root $f })
        let skipped = ($all | length) - ($kept | length)
        if $skipped > 0 {
            log info $"cargo: skipping ($skipped) workspace-member manifests, processing ($kept | length) roots"
        }
        $kept
    } else { [] }

    let node_files = if ("node" in $allowed) {
        let all = (find-manifests "package.json")
        let kept = (dedupe-workspace-members $all { |f| is-node-workspace-root $f })
        let skipped = ($all | length) - ($kept | length)
        if $skipped > 0 {
            log info $"node: skipping ($skipped) workspace-member manifests, processing ($kept | length) roots"
        }
        $kept
    } else { [] }

    let py_files = if ("uv" in $allowed) { find-manifests "pyproject.toml" } else { [] }

    if ($cargo_files | is-empty) and ($node_files | is-empty) and ($py_files | is-empty) {
        warn $"no Cargo.toml, package.json, or pyproject.toml under (pwd)"
        exit 1
    }

    let original_pwd = (pwd)

    # Pre-scan
    let pre = if $cfg.scan { (run-osv "pre-update") } else { {ok: true, vulns: 0, skipped: true} }
    cd $original_pwd

    # Update each ecosystem
    let cargo_results = if not ($cargo_files | is-empty) { (update-cargo $cargo_files $cfg) } else { [] }
    cd $original_pwd
    let node_results = if not ($node_files | is-empty) { (update-node $node_files $cfg) } else { [] }
    cd $original_pwd
    let uv_results = if not ($py_files | is-empty) { (update-uv $py_files $cfg) } else { [] }
    cd $original_pwd

    # Post-scan
    let post = if $cfg.scan { (run-osv "post-update") } else { {ok: true, vulns: 0, skipped: true} }
    cd $original_pwd

    # Post-update build/lint/test
    let checks = if $cfg.tests { (run-checks) } else { {ok: true, runner: "skipped", failed: []} }
    cd $original_pwd

    # Socket (paranoid)
    let socket = if $cfg.socket and (has-cmd "socket") {
        section "Socket"
        {ok: (try-run "socket" { ^socket scan create --json . }), used: true}
    } else if $cfg.socket {
        warn "socket CLI not installed; skipping  (npm i -g socket)"
        {ok: true, used: false}
    } else {
        {ok: true, used: false}
    }

    let all_updates = ($cargo_results | append $node_results | append $uv_results)
    let failed_updates = ($all_updates | where { |r| not $r.ok })

    let report = {
        mode: $cfg.mode
        ok: (
            $pre.ok and $post.ok and $checks.ok and $socket.ok
            and ($failed_updates | is-empty)
        )
        cooldown_days: $cfg.cooldown
        pre_scan: $pre
        updates: $all_updates
        post_scan: $post
        checks: $checks
        socket: $socket
    }

    if $report.ok {
        section "✓ upkg complete"
        exit 0
    } else {
        section "✗ upkg has failures"
        emit-diagnosis $report
        exit 1
    }
}
