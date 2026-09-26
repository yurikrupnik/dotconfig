# Agent notes — dotconfig

Dotfiles managed by a generate→stow pipeline. Read `README.md` for the full
architecture; the rules below are what agents get wrong.

## Cardinal rule

**Never edit `output/`** — it is generator-owned and wiped on every run.
Sources of truth:

- `config/shell/config.toml` — aliases, `[functions.*]` (become bash scripts on
  PATH), `[environment]` (emitted to zsh + nushell)
- `config/scripts/*.nu|sh` — hand-written scripts, copied to `~/.local/bin/<name>`
  with extension stripped (`upkg.nu` → `upkg`, `mcp.nu` → `mcp`)
- `config/brew/Brewfile`, `config/cargo/liner.toml`, `config/node/package.json`,
  `config/uv/tools.txt` — machine packages, installed/refreshed by `update`
- Hand-written stow packages at repo root (`zsh/`, `nushell/`, `zed/`, ...)

After editing generated sources: `just regen` (generate + restow). Hand-written
packages (e.g. `nvim/`, `zed/`) are symlinked — edits are live immediately, no
regen.

## Naming constraints

- The machine refresher is `update` (alias `u`). In **nushell**, `update` is a
  builtin — the generator emits `alias u = ^update` (caret) for any alias whose
  target is a `[functions.*]` name. Bare names shadowing nu builtins
  (`update`, `sort`, `generate`) need `^name` in nu.
- Don't name anything `up` — that's the Upbound CLI (brew `upbound/tap/up`).

## Key commands

- `update` / `u` — refresh all machine packages (brew, rust, cargo, node, uv,
  gcloud) + regen; defined in config.toml `[functions.update]`. It sets
  `on_error = "continue"`, so the generated script runs every step, lists the
  failures at the end, and exits 1 — a flaky upstream no longer skips the
  remaining steps. Other `[functions.*]` keep `set -e` (abort on first failure).
- `upkg` — update *project* deps in $PWD (cargo/node/uv, workspace-aware,
  OSV scans, cooldown; `--fast`/`--paranoid`); source: `config/scripts/upkg.nu`
- `toolbelt` — read-only dashboard of every tool dotconfig installs (brew, cask,
  mise, cargo, node, uv, ~/.local/bin, aliases, functions, scripts, just
  recipes): per-shell usage, status, custom-code ROI, alias gaps. Source:
  `config/scripts/toolbelt.nu`. zsh run counts come from the preexec logger in
  `zsh/.config/zsh/.zshrc` (`~/.local/state/toolbelt/zsh.tsv`) because
  `.zsh_history` is deduped; keep that path in sync with `zsh-log-path`.
  `toolbelt govern` is the read-only security/governance audit (shells, gcp,
  mcp, agents, clusters: management clusters + children via CAPI, Crossplane,
  vcluster, Flux, Argo CD). It reads MCP client paths from `mcp.nu`'s exported
  `TARGETS` — keep that `export const`.
  `toolbelt manage` is the only mutating subcommand: an AI agent (`claude -p`
  with `--json-schema`, or `-a omp`) picks install/uninstall/declare/undeclare
  for rows with `also`/shadowed, missing/unused/rare status or drift; toolbelt
  validates picks against `allowed-actions` and builds the commands/declaration
  edits itself. Plan-only unless `--apply` (confirms each action).
- `devkit` — kind/compose local-env engine. **Not in this repo**: it lives in
  toolkit (`~/shluviza.com/toolkit/apps/devkit`) and is installed by
  `bun nx run devkit:link` (or `devkit:install`) from there, which owns
  `~/.local/bin/devkit`, `~/.local/lib/devkit` and
  `~/.config/nushell/scripts/devkit`. `devkit.toml` here is only this repo's
  per-repo config for it.
- `just doctor` — health check; `just outdated` — preview refresh

## Environment notes

- `NO_PROXY=localhost,127.0.0.1,::1,.test` is set globally because the sfw
  (Socket Firewall) aliases (`cargo`→`sfw cargo`, `bun`, `pnpm`, `uv`) inject
  HTTP(S)_PROXY into children; tests hitting reserved `.test` hosts must bypass.
- nushell `$"..."` interpolation: bare `(word)` executes as a subexpression —
  escape literal parens (`\(s\)`). External commands failing mid-script kill
  nu scripts unless wrapped in `try {}` or `| complete`.
