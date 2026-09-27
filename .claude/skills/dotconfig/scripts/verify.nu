#!/usr/bin/env nu
# Run every dotconfig check on every file: lefthook pre-commit with --all-files
# (gitleaks-staged, shellcheck, nu-check, taplo, generator validate) plus CI's
# `generator` job (generate to a temp dir, syntax-check each generated file).
# Writes nothing under $HOME. `shells.nu generate` still prunes stale package
# dirs from output/, exactly as `just generate` does.
#
#   nu .claude/skills/dotconfig/scripts/verify.nu             # lint + generator
#   nu .claude/skills/dotconfig/scripts/verify.nu --brewfile  # + every Brewfile entry exists (network, slow)
#   nu .claude/skills/dotconfig/scripts/verify.nu --machine   # + scripts/doctor.sh (this machine's install)

# <repo>/.claude/skills/dotconfig/scripts/verify.nu
const REPO = path self | path dirname | path dirname | path dirname | path dirname | path dirname
const SHELLS = "scripts/nu/setup-local-machine/shells.nu"

# Run an external command in the repo; print a ✓/✗ line and, on failure, the
# output tail. Returns true on success.
def check [label: string, argv: list<string>, --vars: record = {}]: nothing -> bool {
    let r = with-env $vars { run-external ($argv | first) ...($argv | skip 1) | complete }
    if $r.exit_code == 0 {
        print $"  ✓ ($label)"
        return true
    }
    print $"  ✗ ($label) \(exit ($r.exit_code)\)"
    $"($r.stdout)\n($r.stderr)" | str trim | lines | last 40 | each {|l| print $"      ($l)" } | ignore
    false
}

def require [cmds: list<string>] {
    let missing = $cmds | where {|c| which $c | is-empty }
    if ($missing | is-not-empty) {
        error make { msg: $"missing on PATH: ($missing | str join ', ') — all are declared in config/brew/Brewfile" }
    }
}

# CI's `generator` job via the same scripts/ci.sh steps ci.yml runs (validate
# already ran in lefthook). Generates into `tmp`, nothing under $HOME.
def generator_checks [tmp: string]: nothing -> list<bool> {
    let vars = { CI_GEN_DIR: $tmp }
    let generated = check "scripts/ci.sh generate (temp dir)" [scripts/ci.sh generate] --vars $vars
    if not $generated {
        return [false]
    }
    [(check "scripts/ci.sh check-generated" [scripts/ci.sh check-generated] --vars $vars)]
}

def main [
    --brewfile  # also run scripts/check-brewfile.sh (queries brew; no taps added)
    --machine   # also run scripts/doctor.sh against this machine's install
] {
    cd $REPO
    # nu exports these to children when running a script file; a nested
    # `nu -c 'nu-check <relative path>'` (lefthook's nu-check job) then resolves
    # the path against this script's dir and reports "file not found".
    hide-env -i FILE_PWD CURRENT_FILE
    require [lefthook shellcheck taplo zsh]

    print "── lefthook pre-commit (all files)"
    mut results = [(check "lefthook run pre-commit --all-files" [lefthook run pre-commit --all-files])]

    print "── generator (CI job)"
    let tmp = mktemp -d
    $results = $results | append (generator_checks $tmp)
    rm -rf $tmp

    if $brewfile {
        print "── Brewfile"
        $results = $results | append (check "scripts/check-brewfile.sh" [scripts/check-brewfile.sh])
    }
    if $machine {
        print "── machine (doctor)"
        $results = $results | append (check "scripts/doctor.sh" [scripts/doctor.sh])
    }

    let failed = $results | where {|ok| not $ok } | length
    print $"\n($results | length) checks, ($failed) failed"
    if $failed > 0 {
        exit 1
    }
}
