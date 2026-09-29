# dotconfig

Personal dotfiles and machine setup. One source of truth for shell configs, packages, and tooling — applied via GNU stow.

## Quick Start

### Fresh Machine

```bash
git clone https://github.com/yurikrupnik/dotconfig.git ~/dotconfig
cd ~/dotconfig
./install.sh
```

This:
1. Installs Homebrew (if missing), puts it on `PATH`, and runs `scripts/brew-preflight.sh --apply` (trust new taps, then `brew bundle`); any bundle failure aborts
2. Installs rustup via the official installer if missing (it is not in the Brewfile), updates the toolchain, adds rust-analyzer
3. Bootstraps [`sfw`](#supply-chain-hardening) (`bun add --global sfw`, the one unproxied install) so every later package step goes through it
4. Generates shell configs and `bin/` scripts from `config/` into `output/`, then symlinks them and the hand-written packages into `$HOME` via GNU stow
5. Installs global cargo tools (`config/cargo/liner.toml`), npm/bun packages (`config/node/package.json`) and uv tools (`config/uv/tools.txt`)

Non-interactive runs refuse to trust new taps; set `BREW_TRUST_NEW_TAPS=1` to allow it. After install, run `just doctor` to verify everything is wired correctly, and `lefthook install` in the repo to enable the git hooks.

### One-liner (from anywhere)

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/yurikrupnik/dotconfig/main/bootstrap.sh)
```

`bootstrap.sh` clones the repo to `~/dotconfig` and runs `./install.sh`.

## Daily Commands

```bash
u                           # Refresh installed packages (brew + rust + cargo + node + uv + gcloud) and restow
                            #   — alias for `update`; resolves to ~/.local/bin/update on every shell

just                        # List all recipes
just doctor                 # Verify install health (symlinks, freshness, dangling links, undeclared installs)
just outdated               # Preview what `u` would refresh

toolbelt                    # Dashboard: every brew/mise/cargo/node/uv tool + alias/function/script,
                            #   per-shell usage, value vs code you maintain, unused tools, alias gaps
toolbelt tools -s brew      # Table view (filters: --source --status --shell; --json on every view)
toolbelt value|gaps|shells|ui  # custom-code ROI · hand-typed repeats · shell cards · interactive browser
toolbelt govern             # Read-only security/governance audit: shells, gcp, mcp, agents, clusters —
                            #   management clusters + the children they created (CAPI/Crossplane/vcluster/Flux/Argo)
toolbelt govern -s clusters -c <ctx>  # one management cluster and its children; --all adds passing checks
toolbelt manage             # AI agent (claude; -a omp) plans installs from the `also`/status/managed columns:
                            #   dedupe managers, install missing, declare/uninstall drift, drop unused (plan only)
toolbelt manage --apply     # same, then confirm each action: runs brew/mise/cargo/bun/uv + edits config/ declarations
toolbelt cost --since 7day  # Per command: runs, failures, total/avg time, last run (zsh run log + nu history);
                            #   --shell zsh|nu|all, --top N; adds calltrace's traced processes + caller → callee edges

# zsh .zsh_history is deduped, so zsh/.config/zsh/.zshrc logs every finished run (start, exit
# status, duration) to ~/.local/state/toolbelt/zsh.tsv (preexec/precmd hooks); nu runs come
# from history.sqlite3.

ctrun <cmd> [args]          # alias for `calltrace run -- <cmd>`: record every process <cmd> spawns (caller → callee tree)
calltrace ls|report|flame|stats  # recorded runs · one run's summary · flame graph · per-command counts across runs
                            #   calltrace lives in toolkit (crates/calltrace): `bun nx run calltrace:install` there

ghfleet                     # Every GitHub repo (you + your orgs) in explore: metadata, settings, config files,
                            #   CI workflows + last run, CD (Argo CD/Flux), Kyverno validations, KEDA scalers
ghfleet scan -m keda        # Table/--json; -o owner,org · --missing kyverno|keda|cd|ci · --cd flux · -c cached
ghfleet show|validate <o/r> # One repo's full record · `kyverno apply` of repo + baseline policies to its manifests
ghfleet add <o/r> [-f params.yaml] [--keda-max 20 …] [--apply] [--context <ctx>]
                            #   generate missing Kyverno ValidatingPolicies / KEDA ScaledObjects from params;
                            #   plan only, --apply opens a PR (or applies to <ctx>, server dry-run when planning)

aicommit [-a omp] [-t KEY-1] [-y]  # `git add .` + AI Conventional Commit from the staged diff, the Jira ticket
                            #   in the branch name (+ its linked Confluence pages) and repo-root README.md/AGENTS.md;
                            #   shows the message, then [y]es/[e]dit/[n]o. Jira/Confluence need JIRA_URL,
                            #   JIRA_EMAIL, JIRA_API_TOKEN (CONFLUENCE_URL defaults to $JIRA_URL/wiki)

just regen                  # Validate config, regenerate output/ from config/ + restow
just stow / unstow          # Re-apply or remove stowed symlinks
just stow-dry               # Preview stow operations
nu scripts/nu/setup-local-machine/shells.nu validate   # Check config.toml + config/scripts only
nu scripts/nu/setup-local-machine/shells.nu stow mise  # Stow selected packages
scripts/check-brewfile.sh   # Every Brewfile tap/brew/cask exists (no installs; --tap adds missing taps)
just ci-tekton              # ci.yml's checks as a Tekton PipelineRun in the devkit Kind cluster (working tree)
```

`u`/`update` is the daily refresher; `./install.sh` is for fresh machines. In nushell, `update` is a builtin, so use `u` (aliased to `^update`) or type `^update`. (Not named `up` — that would shadow the Upbound CLI from the Brewfile.)

## What Gets Managed

| Component | Source of truth | Destination |
|-----------|-----------------|-------------|
| Shell aliases / env vars / sequence-of-command functions | [`config/shell/config.toml`](config/shell/README.md) | Generated per-shell + `~/.local/bin/<name>` |
| Hand-written scripts (any language) | [`config/scripts/`](config/scripts/README.md) | `~/.local/bin/<name>` |
| Brew packages | `config/brew/Brewfile` | System |
| Cargo tools | `config/cargo/liner.toml` | `~/.cargo/bin/` (via [cargo-liner](https://docs.rs/cargo-liner), symlinked from `$CARGO_HOME/liner.toml`) |
| npm/bun globals | `config/node/package.json` | Global node modules |
| uv (Python) tools | `config/uv/tools.txt` | `~/.local/bin/` (via `uv tool install`, isolated per-tool venvs) |
| pnpm config (supply-chain hardening) | `pnpm/` | `~/.config/pnpm/config.yaml` |
| bun config (supply-chain hardening) | `bun/` | `~/.bunfig.toml` |
| zsh config (hand) | `zsh/` | `~/.zshenv`, `~/.config/zsh/.zshrc` |
| zsh config (generated) | `output/zsh/` | `~/.config/zsh/generated.zsh` |
| nushell config (hand) | `nushell/` | `~/.config/nushell/config.nu`, `env.nu` |
| nushell config (generated) | `output/nu/` | `~/.config/nushell/generated.nu` |
| starship prompt | `starship/` | `~/.config/starship/` |
| zed editor | `zed/` | `~/.config/zed/` |
| neovim editor | `nvim/` | `~/.config/nvim/` (see [the learning plan](nvim/.config/nvim/docs/learning-plan.md)) |
| mise global config (empty on purpose) | `mise/` | `~/.config/mise/config.toml` |
| bat syntaxes (Nushell; bat ships none) | `bat/` | `~/.config/bat/syntaxes/` (compiled by `just bat-cache`, part of `just regen`) |

### One owner per tool

Every CLI has exactly one manager: the Brewfile first, then `liner.toml` (crates with no formula), `package.json` (node globals), `tools.txt` (Python CLIs). mise only provides runtimes that a project pins in its own `mise.toml` — its global config stays empty and `mise prune` removes anything no project needs. `just doctor` fails on anything installed through brew, cargo (registry crates), uv or bun that the matching file doesn't declare: declare it or uninstall it.

## How It Works

The repo is organized into three roles:

1. **`config/`** — source for things that get *generated*: shell aliases, env vars, sequence-of-command functions (`config/shell/config.toml`), and hand-written scripts in any language (`config/scripts/`).
2. **`output/`** — generator output. **Do not edit by hand. Not committed to git** — rebuilt from `config/` by `shells.nu generate`.
3. **Top-level hand-written stow packages** — `zsh/`, `nushell/`, `zed/`, `nvim/`, `starship/`, `pnpm/`, `bun/`, `mise/`, `bat/`. These hold config files that are pure source (no generation step). They're committed to git and stowed as-is.

The pipeline:

1. `shells.nu generate` validates `config/shell/config.toml` and `config/scripts/*` (unknown keys, alias/function/script name collisions, shell syntax in aliases, bad env references — every problem is listed, nothing is written), then writes everything to `output/`. `output/bin/` is rebuilt from scratch and any `output/` package it no longer generates is deleted. Aliases named like a nushell builtin (`ls`) are emitted for zsh only.
2. `shells.nu stow` uses GNU stow with `--no-folding` to symlink the generated packages (`zsh`, `nu`, `bin`) and each entry of `HAND_WRITTEN_PACKAGES` (`zed`, `starship`, `zsh`, `nushell`, `pnpm`, `bun`, `nvim`, `mise`, `bat`) into `$HOME`, then deletes symlinks under `$HOME` that point into the repo but no longer resolve (a removed function, script, or package file). A missing package dir or any stow conflict fails the run. Hand-written and generated packages share target directories (e.g. `~/.config/zsh/` gets `.zshrc` from `zsh/` and `generated.zsh` from `output/zsh/`). `just regen` then runs `just bat-cache` (`bat cache --build`): bat only reads compiled syntaxes, and the cache must match the installed bat version.
3. `config/brew/Brewfile`, `config/cargo/liner.toml`, `config/node/package.json`, and `config/uv/tools.txt` declare packages installed by `./install.sh` and refreshed by `u`/`update`.

**On a fresh clone**, `output/` does not exist. `./install.sh` runs `generate` before `stow`, so it bootstraps correctly. If you ever run `just stow` directly on a fresh clone, you'll see an error pointing at `just generate`.

## Supply-chain hardening

Two layers, defense in depth:

1. **Package-manager config** ([`pnpm/.config/pnpm/config.yaml`](pnpm/.config/pnpm/config.yaml), [`bun/.bunfig.toml`](bun/.bunfig.toml)) — 7-day minimum release age (skip versions <1 week old, catches >90% of supply-chain attacks since they're flagged within hours), no lifecycle scripts in bun (`ignoreScripts = true`), no git/tarball sources in pnpm transitive deps (`blockExoticSubdeps`), and `trustPolicy: no-downgrade` so a package losing its trusted-publisher signature fails install.
2. **[Socket Firewall](https://github.com/SocketDev/sfw-free) (`sfw`) at install time** — `config/shell/config.toml` aliases `pnpm`, `bun`, `cargo`, and `uv` through `sfw <name>` for interactive shells. Scripts don't expand aliases, so `u` and the `cargo-install`/`node-install`/`uv-install` recipes call `sfw` explicitly (they fail if it is missing). sfw intercepts TLS with a per-run CA; cargo reads it from `CARGO_HTTP_CAINFO`, cargo-binstall needs `BINSTALL_HTTPS_ROOT_CERTS` (set by the recipe). cargo-binstall is also limited to crate-published binaries or source builds (`BINSTALL_STRATEGIES`, no third-party QuickInstall). Bypass per-command with the bare binary path (e.g. `/opt/homebrew/bin/bun add foo`).
3. **`upkg` batch update flow** ([`config/scripts/upkg.nu`](config/scripts/upkg.nu)) — when updating project deps, runs OSV-Scanner pre/post, gates new versions through `ncu --cooldown 7`, passes `--ignore-scripts` to every PM, and (with `--paranoid` or `--with-sfw`) wraps install calls through `sfw`. The `socket` CLI is installed globally and runs in `--paranoid` mode for a post-install risk audit.

Inspired by [this video](https://www.youtube.com/watch?v=Wq6yMdt11LM).

## Adding new functionality

This repo has **two places** to define a new command: pick the one that fits.

| You want… | Put it in… | Why |
|---|---|---|
| An alias (`k = "kubectl"`) | `config/shell/config.toml` `[aliases]` | One-liner per shell, no logic |
| An env var (`EDITOR = "zed"`) | `config/shell/config.toml` `[environment]` | Exported in every shell |
| "Run these N commands in order" | `config/shell/config.toml` `[functions.X]` | Emits a bash script on `PATH`. See [`config/shell/README.md`](config/shell/README.md). |
| Anything with flags, branches, loops, or structured data | `config/scripts/<name>.<ext>` | Hand-written file in any language. See [`config/scripts/README.md`](config/scripts/README.md). |

The shell user types `<name>` and the shell finds the resulting file on `PATH` — it doesn't care whether your function came from a TOML block or a hand-written nu script.

### Adding packages

```bash
# Homebrew — add to config/brew/Brewfile
echo 'brew "neovim"' >> config/brew/Brewfile
brew bundle --file=config/brew/Brewfile

# Cargo — add to config/cargo/liner.toml under [packages]
just cargo-install

# npm/bun — add to config/node/package.json
just node-install

# uv (Python CLI tools) — add to config/uv/tools.txt (one tool per line)
echo 's-tui' >> config/uv/tools.txt
just uv-install
```

## Checks

- **Git hooks** (`lefthook.yml`, enable with `lefthook install`): gitleaks on staged changes, shellcheck, nu-check, taplo, `shells.nu validate`, and a Conventional Commits `commit-msg` check (`type(scope): subject`).
- **CI** (`.github/workflows/ci.yml`): lint (shellcheck, taplo, nu-check), generator on macOS + Ubuntu (validate, generate to a temp dir, syntax-check + shellcheck every output), Brewfile (`scripts/check-brewfile.sh --tap`), and a gitleaks scan of the full history. Known false positives are fingerprinted in `.gitleaksignore`. Every lint/generator/secrets step is one `scripts/ci.sh <step>` call, shared with Tekton and `verify.nu`.
- **CI on Tekton, locally**: `just ci-tekton` (`scripts/nu/ci-tekton.nu`) runs the lint, generator and secrets jobs as three parallel Tasks (`manifests/tekton/ci.yaml`) in the devkit Kind cluster. It builds `manifests/dockers/ci.Dockerfile` (ci.yml's pinned, sha256-verified tools + the working tree — tracked and untracked, never ignored files), tags it by image ID, `kind load`s it (no registry), starts a PipelineRun, streams `tkn` logs and exits with the run's verdict. One-time setup: `devkit cluster create` then `devkit cluster deps` (Tekton Pipelines LTS comes from `devkit.toml` `[[deps]]`). The Brewfile job stays on GitHub Actions (needs Homebrew on macOS).
- **Machine**: `just doctor`, `just outdated`. `./install.sh` is idempotent — re-running is the simplest end-to-end test.
- **Hooks + CI generator job, locally**: `nu .claude/skills/dotconfig/scripts/verify.nu` runs lefthook's pre-commit jobs on every tracked file plus CI's generator job in a temp dir (`--brewfile` adds the Brewfile check, `--machine` adds doctor). `nu .claude/skills/dotconfig/scripts/platform-audit.nu` lists code tied to one OS (`--events` prints one NDJSON event per hit; `--watch` keeps streaming NDJSON — current hits, then `new`/`resolved` events as tracked files are saved or `git add`/`rm`'d; `--diff` reports new/resolved hits vs `platform-baseline.json` and exits 1 on new ones).
- **Sandboxes** (`scripts/nu/sandbox.nu`, Lima from the Brewfile): `just sandbox-linux` / `just sandbox-mac` clone a pristine Ubuntu 24.04 / macOS VM, copy the working tree in (tracked + untracked, never ignored files like `.env`; no host mounts), run `./install.sh`, open a shell, and delete the VM on exit. `--keep` stops instead of deleting the VM and the next run reuses it (idempotency test); one session per OS at a time, `--fresh` drops it, `--shell` skips install, `--template <lima template>` picks another distro; `just sandbox-clean` removes all sandbox VMs including the cached bases. The first macOS run restores an IPSW (~20 GB). Windows has no automated sandbox (Lima has no Windows guests): use the `utm` cask with Windows 11 ARM.

## File Structure

```
dotconfig/
├── install.sh                          # Fresh machine bootstrap
├── bootstrap.sh                        # Clone + install (for curl piping)
├── justfile                            # Task runner (wraps the scripts below)
├── .editorconfig                       # Cross-editor formatting
├── .claude/skills/                     # Agent skills: dotconfig (manage, verify, port), add-shell-command
├── config/                             # Source of truth (hand-edited)
│   ├── brew/Brewfile                   # Homebrew packages
│   ├── cargo/liner.toml                # Global cargo tools
│   ├── node/package.json               # Global npm/bun packages
│   ├── uv/tools.txt                    # Global Python CLI tools (uv tool install)
│   ├── shell/
│   │   ├── config.toml                 # Aliases / env / sequence-of-command functions
│   │   └── README.md                   # When to use [functions.X]
│   └── scripts/                        # Hand-written scripts (any language)
│       ├── ghfleet.nu                  # → ~/.local/bin/ghfleet (GitHub repos: CI/CD/Kyverno/KEDA audit + add)
│       ├── kgov.nu                     # → ~/.local/bin/kgov (toolbelt govern findings in explore)
│       ├── mcp.nu                      # → ~/.local/bin/mcp
│       ├── nx-run.nu                   # → ~/.local/bin/nx-run
│       ├── toolbelt.nu                 # → ~/.local/bin/toolbelt (tool inventory + usage/value dashboard)
│       ├── upkg.nu                     # → ~/.local/bin/upkg
│       └── README.md                   # When to write a script; bash vs nu
├── manifests/
│   ├── dockers/ci.Dockerfile           # Tekton CI image: ci.yml's toolchain + working tree
│   └── tekton/ci.yaml                  # Tekton Tasks + Pipeline mirroring ci.yml (just ci-tekton)
├── scripts/
│   ├── brew-preflight.sh               # Tap-trust check + brew bundle (bash 3.2-safe)
│   ├── check-brewfile.sh               # Every Brewfile entry exists (CI + local)
│   ├── ci.sh                           # CI steps, shared by ci.yml, Tekton and verify.nu
│   ├── doctor.sh                       # Health check (commands, symlinks, freshness, drift)
│   ├── outdated.sh                     # Preview pending updates (brew/rust/node/uv)
│   └── nu/
│       ├── ci-tekton.nu                # Build CI image, kind load, run the Tekton pipeline (just ci-tekton)
│       ├── sandbox.nu                  # Disposable Lima VMs running install.sh (just sandbox-*)
│       ├── worktree.nu                 # Working-tree tarball (no ignored files) for sandbox + CI image
│       └── setup-local-machine/
│           └── shells.nu               # Generator + stow driver
├── devkit.toml                         # This repo's devkit config (Kind cluster, [[deps]] incl. Tekton)
├── output/                             # Generated; NOT committed to git
│   ├── bin/.local/bin/                 # Functions from TOML + scripts from config/scripts/
│   ├── zsh/.config/zsh/generated.zsh   # Generated zsh aliases + env
│   └── nu/.config/nushell/generated.nu # Generated nu aliases + env
├── zsh/                                # Hand-written zsh source
│   ├── .zshenv
│   └── .config/zsh/.zshrc
├── nushell/.config/nushell/            # Hand-written nu source
│   ├── config.nu
│   └── env.nu
├── starship/.config/starship/          # Hand-written starship prompt
├── zed/.config/zed/                    # Hand-written Zed config
├── nvim/.config/nvim/                  # Hand-written Neovim config (Neovim ≥ 0.12)
│   ├── init.lua                        # Entry point; leader, trainer mode, module order
│   ├── lua/config/                     # options, keymaps, autocmds, lazy bootstrap, LSP
│   ├── lua/plugins/                    # One file per concern (ui, editor, lsp deps, git, …)
│   ├── lsp/<server>.lua                # Native vim.lsp.config server definitions (no mason)
│   ├── lazy-lock.json                  # Committed plugin lockfile — reproducible on a new box
│   └── docs/learning-plan.md           # 4-week plan + keymap reference (<leader>L in nvim)
├── mise/.config/mise/config.toml       # Global mise config: empty; runtimes come from project mise.toml
├── bat/.config/bat/syntaxes/           # Extra bat syntaxes (vendored Nushell .sublime-syntax)
├── lefthook.yml                        # Git hooks (lefthook install)
├── .github/workflows/ci.yml            # CI checks
├── pnpm/.config/pnpm/config.yaml       # pnpm supply-chain hardening (min release age, no exotic subdeps, trust policy)
└── bun/.bunfig.toml                    # bun supply-chain hardening (min release age, ignore lifecycle scripts)
```

## Command Runners

- **`./install.sh`, `./bootstrap.sh`** — pure bash, run before nu exists
- **`u`/`update`** — daily refresh of installed packages (defined in `config.toml`, lives on `PATH`)
- **`./scripts/doctor.sh`, `./scripts/outdated.sh`** — health check + update preview
- **`just <recipe>`** — short aliases for daily commands; requires `brew install just`

There is no Makefile — the bash scripts are the canonical entry points; `just` is just for ergonomics.

## Supported Platforms

- macOS (Apple Silicon and Intel)
