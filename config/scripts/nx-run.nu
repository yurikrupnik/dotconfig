#!/usr/bin/env nu

# Run Nx `run-many` (default) or `affected` across one or more targets.
#
# Examples:
#   nx-run                              # run-many -t build, parallel = logical cpus
#   nx-run build lint test              # multiple targets at once
#   nx-run -a build test                # affected instead of run-many
#   nx-run build --projects api,web     # restrict to specific projects
#   nx-run build -p 4 --skip-nx-cache   # cap parallelism, bypass cache
#   nx-run e2e -c ci                    # nx --configuration=ci
#   nx-run build --prod                 # shorthand for --configuration=production
#   nx-run build --dry-run              # print the command without running it

export def main [
    ...targets: string                  # one or more nx targets (default: build)
    --affected(-a)                      # use `nx affected` instead of `run-many`
    --parallel(-p): int                 # max parallel tasks (default: logical cpus)
    --projects: string                  # comma-separated list — run-many only
    --exclude: string                   # comma-separated list of projects to skip
    --configuration(-c): string         # nx --configuration value (e.g. ci, production)
    --prod                              # shorthand for --configuration=production
    --skip-nx-cache                     # bypass the nx cache
    --verbose(-v)                       # pass --verbose to nx
    --dry-run                           # print the command and exit
] {
    let targets = if ($targets | is-empty) { ["build"] } else { $targets }
    let parallel = if $parallel == null { sys cpu | length } else { $parallel }
    let cfg_raw = $configuration | default ""

    if $affected and (($projects | default "" | is-not-empty) or ($exclude | default "" | is-not-empty)) {
        error make {msg: "--projects/--exclude only apply to run-many, not --affected"}
    }
    if $prod and ($cfg_raw | is-not-empty) and $cfg_raw != "production" {
        error make {msg: $"--prod conflicts with --configuration=($cfg_raw)"}
    }
    if $parallel < 1 {
        error make {msg: $"--parallel must be >= 1, got ($parallel)"}
    }

    let sub = if $affected { "affected" } else { "run-many" }
    let cfg = if $prod { "production" } else { $cfg_raw }

    mut args = ["nx" $sub "-t" ($targets | str join ",") $"--parallel=($parallel)"]
    if ($projects | default "" | is-not-empty) { $args = ($args | append $"--projects=($projects)") }
    if ($exclude  | default "" | is-not-empty) { $args = ($args | append $"--exclude=($exclude)") }
    if ($cfg | is-not-empty)                   { $args = ($args | append $"--configuration=($cfg)") }
    if $skip_nx_cache                          { $args = ($args | append "--skip-nx-cache") }
    if $verbose                                { $args = ($args | append "--verbose") }

    let cmd = (["bun"] | append $args)
    print $"(ansi green_bold)==>(ansi reset) ($cmd | str join ' ')"
    if $dry_run { return }

    ^($cmd.0) ...($cmd | skip 1)
}
