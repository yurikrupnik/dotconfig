---
name: dotconfig
description: Manage the dotconfig dotfiles repo end to end — packages (brew/cargo/bun/uv/mise), stow packages, shell and editor config, the shells.nu generator, doctor/lefthook/CI checks, and macOS/Linux/Windows portability. Use for any change, review, audit, or port of this repo; for a single alias/function/script defer to the add-shell-command skill.
---

# Managing dotconfig

The repo is the spec. This skill says where to look, what the repo has
already learned the hard way, and how to prove a change. When this file and
the repo disagree, the repo wins — fix this file in the same change.

## 1. Learn before editing

Read only what the task touches; grep the named symbols instead of trusting
memorized values.

| Question | Source of truth |
|---|---|
| Architecture, pipeline, command list | `README.md` (How It Works, Checks), `AGENTS.md` |
| Generator contract (what is valid, what gets stowed) | `scripts/nu/setup-local-machine/shells.nu`: `HAND_WRITTEN_PACKAGES`, `GENERATED_PACKAGES`, `TOP_LEVEL_KEYS`, `FUNCTION_KEYS`, `ALIAS_FORBIDDEN`, `ENV_ALLOWED_REFS`, `LINK_ROOTS` |
| Aliases / env / functions | `config/shell/config.toml`, `config/shell/README.md` |
| Scripts on PATH | `config/scripts/*`, `config/scripts/README.md` |
| Packages | `config/brew/Brewfile`, `config/cargo/liner.toml`, `config/node/package.json`, `config/uv/tools.txt` |
| What "passing" means | `lefthook.yml` (local, staged), `.github/workflows/ci.yml` (authoritative), `scripts/doctor.sh` (machine state) |
| Formatting | `.editorconfig` (LF, 4 spaces; 2 for toml/json/yaml/nu/Brewfile) |
| Commit style in practice | `git log --format=%s -30` |
| OS coupling | `nu .claude/skills/dotconfig/scripts/platform-audit.nu` (`--events` NDJSON per hit, `--watch` live NDJSON new/resolved as files change, `--diff` vs `scripts/platform-baseline.json`) |

Pipeline:

1. `shells.nu generate`: `config/shell/config.toml` + `config/scripts/*` → validated → `output/{zsh,nu,bin}` (gitignored).
2. `shells.nu stow`: `output/{zsh,nu,bin}` + hand-written packages (`zsh/ nushell/ zed/ nvim/ starship/ pnpm/ bun/ mise/`) → symlinks in `$HOME`; dangling repo links swept.
3. Package manifests (`config/brew`, `cargo`, `node`, `uv`) → `./install.sh` (fresh machine) / `update` = `u` (daily, ends with `just regen`).

## 2. Where a change goes

| You want | Edit | Apply |
|---|---|---|
| Alias, env var, command sequence, script on PATH | follow `skill://add-shell-command` | `just regen` |
| A CLI | Brewfile first; else `liner.toml` (crate, no formula) / `package.json` (node global) / `tools.txt` (Python CLI). Exactly one owner. | `just brew-install` / `cargo-install` / `node-install` / `uv-install` |
| A language runtime | the *project's* `mise.toml` — never `mise/.config/mise/config.toml` (stays empty) | `mise install` in the project |
| A config file under `~/…` for a new app | new top-level package mirroring `$HOME` (`foo/.config/foo/…`) + append to `HAND_WRITTEN_PACKAGES` + a `check_stow` line in `scripts/doctor.sh` + README "What Gets Managed" row + File Structure tree | `just stow-dry`, then `just stow` |
| Shell startup (PATH, tool init, hooks) | **both** `zsh/.config/zsh/.zshrc` and `nushell/.config/nushell/{env,config}.nu` — keep them in parity (same PATH order, same tools) | new shell / `exec zsh` |
| Editor config | `nvim/`, `zed/` (live via symlink, no regen) | reopen editor |
| MCP servers for AI clients | `config/mcp/servers.json`; client paths live in `mcp.nu`'s exported `TARGETS` (toolbelt imports it — keep it `export const`) | `mcp` |
| A new check | `lefthook.yml` **and** a `scripts/ci.sh` step called from `ci.yml` + `manifests/tekton/ci.yaml` (hooks mirror CI on staged files; Tekton mirrors ci.yml) | `verify.nu`; `just ci-tekton` |
| CI tool version | `ci.yml` `env:`/install-action pins **and** `manifests/dockers/ci.Dockerfile` ARGs + per-arch sha256 | `just ci-tekton` |
| Machine health assertion | `scripts/doctor.sh` | `just doctor` |
| devkit behavior | not here — toolkit repo; only `devkit.toml` lives here | — |
| Secrets | never tracked. `tmp/`, `.env` are gitignored scratch; gitleaks runs pre-commit + full history in CI | — |

Never edit `output/`: generator-owned, gitignored, wiped each run.

## 3. Invariants (each one broke something before)

- **One manager per CLI.** `just doctor` fails on undeclared brew/cargo/uv/bun installs: declare or uninstall, never ignore.
- **Prerelease crates** need `{ version = "<major>", skip-check = true }` in liner.toml or `u` reinstalls them every run.
- **Bash = macOS stock 3.2**: no `mapfile`, `declare -A`, `${x,,}`; expand arrays as `${a[@]+"${a[@]}"}` under `set -u`. CI runs `/bin/bash -n` on every generated script.
- **Nushell**: bare `(word)` in `$"…"` executes — escape `\(…\)`; failing externals kill the script unless `try {}` / `| complete`; `math sum` on empty input errors (`append 0`). Scripts run as files export `FILE_PWD`/`CURRENT_FILE` to children.
- **Names**: never `up` (Upbound CLI). Names that are nu builtins (`update`, `sort`, `generate`) need `^name` in nu; aliases named like a builtin (`ls`) are zsh-only. Alias/function/script names must be unique (validate enforces).
- **Aliases are plain commands**: `'`, `$(`, backticks, `&&`, `||`, `;`, `|` → `[functions.*]`.
- **Env values** may reference only `$HOME` and `$DOTCONFIG_DIR`.
- **Supply chain**: scripts don't expand aliases, so every network package step in a script/recipe calls `sfw` explicitly (`require-sfw` in the justfile). `NO_PROXY` covers loopback + `.test`. bun/pnpm keep 7-day minimum release age; cargo-binstall stays on `BINSTALL_STRATEGIES="crate-meta-data,compile"`.
- **Failure policy**: `[functions.update]` is `on_error = "continue"` (runs all steps, lists failures, exits 1); everything else aborts on first failure. `install.sh` must stay idempotent — re-running it is the end-to-end test.
- **Stow**: `--no-folding`; `shells.nu stow` sweeps dangling repo links under `LINK_ROOTS`. A new link root (e.g. a new `~/.foo` dir) must be added there and to doctor's dangling scan.
- **Portable paths**: no `/Users/<name>` in tracked files (doctor fails); derive the repo root from the script location (`path self`, `BASH_SOURCE`) with `DOTCONFIG_DIR` override.
- **CI hygiene**: actions pinned by full SHA with `# vX` comment, tool versions pinned in `env:`, downloads sha256-verified, `permissions: contents: read`, `persist-credentials: false`.
- **gitleaks**: a new false positive goes in `.gitleaksignore` by fingerprint, only after confirming the value is not a real secret.

## 4. Cross-platform rules (target: macOS, Linux, Windows)

Current state is macOS-first; details, the per-OS path table, and the porting
playbooks are in `references/platforms.md`. Every change must hold these:

1. **No new unguarded OS coupling.** A macOS-only line needs an OS guard and the Linux/Windows equivalent (or an explicit `# macOS-only: <reason>` comment). `platform-audit.nu --diff` must exit 0 (no hits beyond `scripts/platform-baseline.json`, keyed by rule + file + line text). When a change removes coupling, run `--update-baseline` in the same change so the baseline only ratchets down. The audit reads `git ls-files`: `git add` new files first, or code moved into an untracked file looks "resolved". `manifests/` (Linux container images, k8s/Tekton YAML) is excluded — it runs in Kind nodes whatever the host OS.
2. **Detect, don't assume.** bash: `case "$(uname -s)" in Darwin) … ;; Linux) … ;; MINGW*|MSYS*|CYGWIN*) … ;; esac`. nu: `match $nu.os-info.name { "macos" => …, "linux" => …, "windows" => … }`.
3. **Resolve paths at runtime.** `brew --prefix` (never `/opt/homebrew`), `$nu.home-path`, `path join` (never string-concat `/`), per-OS app dirs from the table in `references/platforms.md`.
4. **Prefer nu for new logic** — it runs natively on all three OSes; bash only for pre-nu bootstrap and generated `[functions.*]`.
5. **POSIX intersection** in shell: no `sed -i`, `readlink -f`, `stat -c/-f`, `date -d`, `grep -P`; use `awk`, temp file + `mv`, or nu.
6. **Packages per OS, one owner per OS.** Formulae work on macOS + Linuxbrew. GUI apps: `cask "x" if OS.mac?` (suffix form — the repo's line parsers in `brew-preflight.sh`, `check-brewfile.sh`, `toolbelt.nu` keep matching). Windows packages go in a Windows manifest (see playbook), never in the Brewfile.
7. **LF everywhere** (`.editorconfig`); a Windows port adds `.gitattributes` `* text=auto eol=lf`.
8. **Supported = CI-proven.** An OS is supported only once `ci.yml` runs the generator job on it (macOS + Ubuntu today) and its playbook is done. Don't widen README "Supported Platforms" without that. Exercise a port locally first with `just sandbox-linux` / `just sandbox-mac` (Sandboxes in `references/platforms.md`).

## 5. Workflow

1. Learn (section 1) → pick the target (section 2) → check invariants (sections 3–4).
2. Edit sources only. Hand-written packages are live immediately; `config/` changes need `just regen`.
3. Verify with the table below — targeted first, then the full suite, then smoke the real thing (run the command, open the shell); a passing lint is not proof.
4. Docs in the same change: README (What Gets Managed, File Structure, Daily Commands), AGENTS.md when an agent-relevant rule changes, `config/*/README.md` for their area, and this skill when a convention changes.
5. Commit: Conventional Commits `type(scope): subject`, one concern per commit (lefthook `commit-msg` enforces the format). Types: feat fix chore docs refactor perf test ci build revert. Scopes seen: `shells`, `toolbelt`, `update`, `install`, `brew`, `node`, `nvim`, `devkit`, `ci`.

| Changed | Targeted check |
|---|---|
| `*.sh`, justfile bash | `shellcheck <f>`; `/bin/bash -n <f>` |
| `*.nu` | `nu -n -c 'nu-check --debug <f>'` |
| `*.toml` | `taplo check <f>` |
| `config.toml`, `config/scripts/*` | `nu scripts/nu/setup-local-machine/shells.nu validate`; `just regen`; `zsh -ic '<name>'`, `nu -l -c '<name>'` |
| Brewfile | `scripts/check-brewfile.sh`; `just brew-preflight` |
| Stow package / new link | `just stow-dry` → `just stow` → `just doctor` |
| `install.sh` / `bootstrap.sh` | re-run `./install.sh` (idempotent); fresh machine: `just sandbox-linux` / `just sandbox-mac` (`--keep` twice = idempotency on a clean VM) |
| `scripts/ci.sh`, `manifests/tekton/`, `manifests/dockers/ci.Dockerfile` | `just ci-tekton` (needs `devkit cluster create` + `devkit cluster deps`) |
| Anything | `nu .claude/skills/dotconfig/scripts/verify.nu` (`--brewfile` network, `--machine` doctor) |

`verify.nu` = lefthook pre-commit on all tracked files + CI's generator job in
a temp dir. Untracked new files are outside `--all-files`; check them with the
targeted command.

## 6. Reviewing / auditing the repo

Run, in order, and report findings with `file:line`:

1. `nu .claude/skills/dotconfig/scripts/verify.nu --brewfile --machine`
2. `nu .claude/skills/dotconfig/scripts/platform-audit.nu --summary` (then drill into a rule; `--diff` for regressions vs baseline, `--events` for NDJSON to pipe into jq/nu, `--watch` to stream new/resolved hits while editing)
3. `toolbelt govern` (security/governance) and `toolbelt` (unused / shadowed / drift) — read-only
4. `just outdated` — pending updates
5. Drift between docs and code: README tables vs `HAND_WRITTEN_PACKAGES`, doctor `check_stow` list vs stowed packages, lefthook jobs vs CI steps.
