#!/usr/bin/env nu
# Read-only: list code in tracked files that ties dotconfig to one OS.
# Every hit is a place that needs an OS guard or a per-OS equivalent before the
# repo can claim macOS + Linux + Windows support (see ../references/platforms.md).
# Heuristic: comment lines are skipped, but hits already inside an OS guard
# (e.g. install.sh's Darwin branch) still print — judge each one.
#
# Each hit is an event record: rule, blocks, file, line, col, match, text, fix.
# `--diff` compares hits to platform-baseline.json (next to this script), keyed
# by rule + file + trimmed line text so unrelated edits that shift line numbers
# don't count as new hits. Exit 1 on new hits; resolved ones mean the baseline
# can be ratcheted down with `--update-baseline`.
#
# `--watch` streams NDJSON until interrupted: every current hit as
# `change: "present"`, then `new` / `resolved` events as tracked files are saved,
# deleted, or `git add`/`rm`/checkout changes the tracked set (.git/index).
#
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu                   # all rules
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu -c windows        # one rule set
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --summary         # counts only
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --events          # NDJSON, one event per hit
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --watch           # NDJSON stream, live new/resolved
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --diff            # new/resolved vs baseline
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --diff --events   # NDJSON, one event per change
#   nu .claude/skills/dotconfig/scripts/platform-audit.nu --update-baseline # rewrite the baseline

# <repo>/.claude/skills/dotconfig/scripts/platform-audit.nu
const REPO = path self | path dirname | path dirname | path dirname | path dirname | path dirname
const BASELINE = path self | path dirname | path join platform-baseline.json

# Not part of the dotfiles pipeline (generated KCL models; manifests/ = Linux
# container images + k8s/Tekton YAML, which run in Kind nodes whatever the host
# OS) or this skill itself.
const EXCLUDE_PREFIXES = ["scripts/kcl/", "manifests/", ".claude/skills/dotconfig/"]

# `blocks`: which OS the hit breaks. Patterns are Rust regex, matched per line.
const RULES = [
    {
        id: "mac-path"
        blocks: "linux windows"
        pattern: '/opt/homebrew|/usr/local/Cellar|Library/Application Support|/Users/[A-Za-z]'
        fix: "resolve at runtime: `brew --prefix`, $nu.home-path, per-OS path table"
    }
    {
        id: "mac-command"
        blocks: "linux windows"
        pattern: '\b(pbcopy|pbpaste|osascript|launchctl|xcode-select|caffeinate|mdfind|diskutil)\b|\bdefaults (read|write)\b|"open"'
        fix: "OS guard; Linux xdg-open/wl-copy, Windows start/clip"
    }
    {
        id: "cask"
        blocks: "linux windows"
        # Casks already suffixed with `if OS.mac?` (the fix below) are not hits.
        pattern: '^\s*cask "[^"]*"\s*(#|$)'
        fix: "suffix `if OS.mac?`; declare the Linux/Windows equivalent in that OS's manifest"
    }
    {
        id: "gnu-bsd"
        blocks: "linux"
        pattern: '\bsed -i\b|\breadlink -f\b|\bstat -[cf]\b|\bdate -d\b|\bmapfile\b|\bdeclare -A\b|\bgrep -P\b'
        fix: "POSIX intersection of GNU + BSD flags; bash 3.2 (macOS /bin/bash)"
    }
    {
        id: "posix-only"
        blocks: "windows"
        pattern: '\bstow\b|\bchmod\b|\bln -s|\breadlink\b|/bin/(ba)?sh\b|#!/usr/bin/env (bash|sh)\b|\bzsh\b'
        fix: "native Windows has no stow/symlink-by-default/bash; see platforms.md Windows playbook"
    }
]

def tracked_files []: nothing -> list<string> {
    ^git -C $REPO ls-files
        | lines
        | where {|f|
            not ($f | str ends-with ".md") and ($EXCLUDE_PREFIXES | all {|p| not ($f | str starts-with $p) })
        }
}

# One event per (line, rule) match in one tracked file; [] if deleted or binary.
def scan_file [file: string, rules: list<record>]: nothing -> list<record> {
    let path = $REPO | path join $file
    if not ($path | path exists) {
        return []
    }
    let content = open --raw $path
    if ($content | describe) != "string" {
        return []
    }
    $content | lines | enumerate | where {|row| $row.item !~ '^\s*(#|--|//)' } | each {|row|
        $rules | where {|r| $row.item =~ $r.pattern } | each {|r|
            let m = $row.item | parse --regex ('(?<m>' + $r.pattern + ')') | get 0.m
            {
                rule: $r.id
                blocks: ($r.blocks | split row " ")
                file: $file
                line: ($row.index + 1)
                col: (($row.item | str index-of --grapheme-clusters $m) + 1)
                match: $m
                text: ($row.item | str trim)
                fix: $r.fix
            }
        }
    } | flatten
}

def hits [rules: list<record>]: nothing -> list<record> {
    tracked_files | each {|file| scan_file $file $rules } | flatten
}

# Baseline form: occurrences per rule + file + trimmed text (line numbers drift).
def tally []: list<record> -> list<record> {
    select rule file text | uniq --count | each {|r| $r.value | insert count $r.count } | sort-by file rule text
}

def count_of [rows: list<record>, key: record]: nothing -> int {
    $rows | where rule == $key.rule and file == $key.file and text == $key.text | get count | append 0 | math sum
}

# Hits in `found` beyond the per-key counts in `known` (tally form), in scan order.
def excess [found: list<record>, known: list<record>]: nothing -> list<record> {
    $found | tally | each {|c|
        let k = count_of $known $c
        if $c.count > $k {
            $found | where rule == $c.rule and file == $c.file and text == $c.text | skip $k
        }
    } | flatten
}

# New events (excess over baseline per key) and resolved keys (baseline excess over current).
def diff_baseline [found: list<record>, rules: list<record>]: nothing -> record {
    if not ($BASELINE | path exists) {
        error make { msg: $"no baseline at ($BASELINE); create it with --update-baseline" }
    }
    let ids = $rules | get id
    let base = open $BASELINE | where rule in $ids
    let current = $found | tally
    let new = excess $found $base | insert change new
    let resolved = $base | each {|b|
        let now = count_of $current $b
        if $b.count > $now {
            $b | update count ($b.count - $now) | insert change resolved
        }
    }
    { new: $new, resolved: $resolved, baseline: ($base | get count | append 0 | math sum) }
}

def emit_ndjson [events: list<record>] {
    let commit = ^git -C $REPO rev-parse --short HEAD | complete | get stdout | str trim
    let meta = { ts: (date now | format date "%+"), commit: $commit }
    for e in $events {
        print ($e | merge $meta | to json --raw)
    }
}

# Stream hits as NDJSON forever: the current set, then per-file new/resolved deltas.
# git swaps the index in via index.lock; the watcher reports only the lock, so
# either path means the tracked set may have changed. In a linked worktree the
# index lives outside $REPO: restart after git add/rm there.
def watch_events [rules: list<record>] {
    let index = ^git -C $REPO rev-parse --path-format=absolute --git-path index | str trim
    let index_paths = [$index $"($index).lock"]
    mut tracked = tracked_files
    mut state = $tracked | each {|f| scan_file $f $rules } | flatten
    emit_ndjson ($state | insert change present)
    for ev in (watch $REPO --quiet) {
        let abs = [$ev.path $ev.new_path] | compact
        let paths = $abs | each {|p| try { $p | path relative-to $REPO } } | compact
        let known = $tracked
        let files = if ($abs | any {|p| $p in $index_paths }) {
            # git add/rm/mv/checkout: rescan everything tracked now or before.
            $tracked = tracked_files
            $tracked | append ($state | each {|h| $h.file }) | uniq
        } else {
            $paths | where {|p| $p in $known }
        }
        for f in $files {
            let before = $state | where file == $f
            let after = if $f in $tracked { scan_file $f $rules } else { [] }
            let changes = excess $after ($before | tally) | insert change new
                | append (excess $before ($after | tally) | insert change resolved)
            if ($changes | is-not-empty) {
                $state = $state | where file != $f | append $after
                emit_ndjson $changes
            }
        }
    }
}

def clip [s: string]: nothing -> string {
    $s | str substring --grapheme-clusters 0..119
}

def main [
    --category (-c): string  # only rules that block this OS: linux | windows
    --summary                # per-rule counts only
    --events                 # NDJSON on stdout: one event per hit (with --diff: per new/resolved hit)
    --watch                  # NDJSON stream until Ctrl-C: current hits, then live new/resolved per saved file
    --diff                   # new/resolved hits vs platform-baseline.json; exit 1 on new hits
    --update-baseline        # rewrite platform-baseline.json from the current hits (all rules)
] {
    if $update_baseline and ($category != null or $summary or $events or $diff or $watch) {
        error make { msg: "--update-baseline takes no other flags (the baseline always covers all rules)" }
    }
    if $summary and ($events or $diff or $watch) {
        error make { msg: "--summary cannot be combined with --events, --diff or --watch" }
    }
    if $watch and $diff {
        error make { msg: "--watch cannot be combined with --diff" }
    }
    let rules = if $category == null {
        $RULES
    } else {
        $RULES | where {|r| $category in ($r.blocks | split row " ") }
    }
    if ($rules | is-empty) {
        error make { msg: $"unknown --category ($category); use linux or windows" }
    }

    if $watch {
        watch_events $rules
        return
    }

    let found = hits $rules

    if $update_baseline {
        let baseline = $found | tally
        $baseline | to json --indent 2 | save --force $BASELINE
        print $"wrote ($baseline | get count | append 0 | math sum) hits \(($baseline | length) keys\) to ($BASELINE)"
        return
    }

    if $diff {
        let d = diff_baseline $found $rules
        if $events {
            emit_ndjson ($d.new | append $d.resolved)
        } else {
            for e in $d.new {
                print $"+ ($e.file):($e.line)  [($e.rule)] (clip $e.text)"
            }
            for e in $d.resolved {
                print $"- ($e.file)  [($e.rule)] (clip $e.text)  ×($e.count)"
            }
            let resolved_n = $d.resolved | get count | append 0 | math sum
            print $"($d.new | length) new, ($resolved_n) resolved vs baseline \(($d.baseline) hits\)"
            if $resolved_n > 0 and ($d.new | is-empty) {
                print "ratchet down: nu .claude/skills/dotconfig/scripts/platform-audit.nu --update-baseline"
            }
        }
        if ($d.new | is-not-empty) {
            exit 1
        }
        return
    }

    if $events {
        emit_ndjson $found
        return
    }

    let counts = $rules | each {|r|
        { rule: $r.id, blocks: $r.blocks, hits: ($found | where rule == $r.id | length), fix: $r.fix }
    }

    if $summary {
        return $counts
    }
    for r in ($counts | where hits > 0) {
        print $"\n── ($r.rule) · blocks ($r.blocks) · ($r.hits) hits — fix: ($r.fix)"
        $found | where rule == $r.rule | each {|h| print $"  ($h.file):($h.line)  (clip $h.text)" } | ignore
    }
    print $"\n($found | length) platform-bound lines in ($found | get file | uniq | length) files"
}
