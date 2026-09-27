# Platform support — macOS, Linux, Windows

Goal: one repo that sets up macOS, Linux, and Windows machines. Today it is
macOS-first. Re-derive the coupling list any time with:

```bash
nu .claude/skills/dotconfig/scripts/platform-audit.nu --summary      # counts per rule
nu .claude/skills/dotconfig/scripts/platform-audit.nu -c linux       # what blocks Linux
nu .claude/skills/dotconfig/scripts/platform-audit.nu -c windows     # what blocks native Windows
nu .claude/skills/dotconfig/scripts/platform-audit.nu --events      # NDJSON: one event per hit
nu .claude/skills/dotconfig/scripts/platform-audit.nu --watch       # NDJSON stream: current hits, then live new/resolved; Ctrl-C to stop
nu .claude/skills/dotconfig/scripts/platform-audit.nu --diff        # new/resolved vs scripts/platform-baseline.json; exit 1 on new
nu .claude/skills/dotconfig/scripts/platform-audit.nu --update-baseline  # after removing coupling
```

Legend: ✓ works and CI-proven · ~ expected to work, not CI-proven · ✗ broken or absent.
Items marked *(verify)* are upstream behavior not yet exercised in this repo;
confirm on a real machine before relying on them.

## Status matrix

| Layer | macOS | Linux | Windows (native) |
|---|---|---|---|
| `bootstrap.sh` / `install.sh` | ✓ | ~ `find_brew` knows Linuxbrew paths; install.sh installs Homebrew's apt/dnf/pacman prerequisites | ✗ bash-only, no PowerShell entry point |
| Brewfile formulae | ✓ | ~ via Linuxbrew (`kusion` guarded: no arm64 Linux build) | ✗ no Windows manifest |
| Brewfile casks | ✓ | ~ macOS-only casks guarded `if OS.mac?`; CLI casks + warp/obsidian/lm-studio install on Linux | ✗ |
| cargo / bun / uv manifests | ✓ | ~ OS-neutral | ~ OS-neutral; `sfw` on Windows *(verify)* |
| `shells.nu validate/generate` | ✓ CI `macos-latest` | ✓ CI `ubuntu-latest` | ✗ calls `^chmod`; emits bash for `[functions.*]` |
| Linking (`shells.nu stow`) | ✓ GNU stow | ~ GNU stow | ✗ no stow; symlinks need Developer Mode |
| zsh config | ✓ | ✗ `.zshrc` hardcodes `/opt/homebrew` (brew shellenv, gcloud) | n/a |
| nushell config | ✓ | ~ loads, but brew env only for `/opt/homebrew` | ✗ untested; see config-dir note |
| nvim / zed / starship | ✓ | ~ same `~/.config` layout | ✗ different app dirs |
| `scripts/doctor.sh` | ✓ | ~ (`/Users/<name>` check misses `/home/<name>`) | ✗ bash |
| `mcp.nu` targets | ✓ | ~ `claude-desktop` path is macOS-only | ✗ |
| CI | lint (ubuntu), generator + Brewfile (macos) | lint + generator (ubuntu) | none |

## Per-OS locations

| Thing | macOS | Linux | Windows (native) | Resolve with |
|---|---|---|---|---|
| Homebrew prefix | `/opt/homebrew` (arm64), `/usr/local` (Intel) | `/home/linuxbrew/.linuxbrew`, `~/.linuxbrew` | — | `brew --prefix`; locate brew like `install.sh` `find_brew` |
| nushell config dir | `~/Library/Application Support/nushell` unless `XDG_CONFIG_HOME` is set in nu's *launching* env | `~/.config/nushell` | `%APPDATA%\nushell` unless `XDG_CONFIG_HOME` set | `$nu.default-config-dir` |
| nvim config | `~/.config/nvim` | `~/.config/nvim` | `%LOCALAPPDATA%\nvim` (honors `XDG_CONFIG_HOME` *(verify)*) | `nvim --headless +'echo stdpath("config")' +q` |
| zed config | `~/.config/zed` | `~/.config/zed` | `%APPDATA%\Zed` *(verify)* | — |
| starship | `STARSHIP_CONFIG` (set in `env.nu`) | same | same | — |
| user bin on PATH | `~/.local/bin` | `~/.local/bin` | `%USERPROFILE%\.local\bin` (add to user PATH) | — |
| Claude desktop MCP | `~/Library/Application Support/Claude/` | no official app | `%APPDATA%\Claude\` | — |
| Open URL | `open` | `xdg-open` | `start ""` / `explorer` | — |
| Clipboard | `pbcopy` / `pbpaste` | `wl-copy` / `xclip` | `clip` / `Get-Clipboard` | — |

Observed on this macOS machine: `$nu.default-config-dir` is `~/.config/nushell`
only because zsh exports `XDG_CONFIG_HOME` (generated from `config.toml`);
with it unset nu reads `~/Library/Application Support/nushell`. Setting
`XDG_CONFIG_HOME=$HOME/.config` *before* nu starts, on every OS, is what keeps
the `$HOME`-mirror stow layout portable.

## Known coupling points

Run the audit for the live list; the structural ones:

- `zsh/.config/zsh/.zshrc`: `eval "$(/opt/homebrew/bin/brew shellenv)"`, gcloud `path.zsh.inc` under `/opt/homebrew/share`.
- `nushell/.config/nushell/env.nu`: brew env only if `/opt/homebrew/bin/brew` exists; JetBrains Toolbox path under `~/Library`.
- `config/shell/config.toml` `[environment]`: `BROWSER = "open"`. The generator emits one value for every OS — there is no per-OS env table.
- `config/brew/Brewfile`: casks without a Linux build carry `if OS.mac?` (zoom/kusion: `|| Hardware::CPU.intel?`). Never add explicit `tap` lines for taps whose formulae fail `brew tap`'s trusted-formula verification (kcl-lang, KusionStack); the tap implied by `brew "o/t/f"` skips it.
- `bootstrap.sh`: `xcode-select` (already guarded by `uname -s = Darwin`).
- `config/scripts/mcp.nu` `TARGETS.claude-desktop`: macOS path.
- `scripts/nu/setup-local-machine/shells.nu`: `^chmod +x` in `generate_bin_scripts` / `generate_user_scripts`; `^stow`, `^find`, `^readlink` in stow + dangling sweep.
- `scripts/doctor.sh`: bash; required commands include `brew stow`; `/Users/<name>` check only.
- `.github/workflows/ci.yml`: generator + Brewfile jobs are `macos-latest` only.

## Playbook: Linux (native, Linuxbrew)

Order matters: make it work, then prove it in CI, then claim it.

1. **Brew env from any prefix.** `.zshrc`: locate brew with the same candidate list as `install.sh` `find_brew`, then `eval "$("$brew" shellenv)"`; gcloud includes from `"$(brew --prefix)/share/google-cloud-sdk"`. `env.nu`: first existing of `/opt/homebrew/bin/brew`, `/usr/local/bin/brew`, `/home/linuxbrew/.linuxbrew/bin/brew`, derive `HOMEBREW_PREFIX` from it.
2. ~~**Casks.** Suffix each with `if OS.mac?`~~ done for every cask Homebrew skips or rejects on Linux. GUI apps on Linux get no owner unless a Linux manifest is added deliberately (e.g. `config/flatpak/apps.txt`) — declare it in README "One owner per tool" and doctor drift if you add one.
3. **`BROWSER`.** Move out of `[environment]` into the OS-guarded shell rc files, or drop it (`xdg-open` is the Linux default). Do not add per-OS logic to the generator for one variable.
4. **doctor.** Hardcoded-home check matches `/Users/<name>` and `/home/<name>`.
5. **mcp.nu.** `claude-desktop` target only when `$nu.os-info.name == "macos"` (Windows path in the Windows playbook).
6. **CI.** ~~Generator matrix `[macos-latest, ubuntu-latest]`~~ done (ubuntu leg installs zsh; `/bin/bash -n` there is bash 5, the macOS leg keeps the 3.2 check). Keep the Brewfile job on macOS unless Linuxbrew is exercised too.
7. **Claim it.** README "Supported Platforms" lists Linux; this file's matrix flips to ✓.

## Sandboxes (local, disposable)

Exercise a port before CI can: `scripts/nu/sandbox.nu` (Lima, declared in the
Brewfile) clones a pristine VM, copies in the working tree (git tracked +
untracked, never ignored files; no host mounts), runs `./install.sh`, then
opens a shell. Bases (`dotconfig-<os>-base-<template>`) are built once and
cloned per run.

| Target | Command | Notes |
|---|---|---|
| Linux | `just sandbox-linux` (`--template debian-13`, `fedora`, … any Lima template) | Guest arch = host arch (aarch64 on Apple Silicon); the guest is a stock cloud image, so install.sh's step 0 (apt/dnf/pacman prerequisites) is exercised |
| macOS | `just sandbox-mac` | Apple Silicon host only; first run restores an IPSW (~20 GB). sudo uses the Lima-generated `~/password` via `SUDO_ASKPASS` |
| Windows | manual: `utm` cask + Windows 11 ARM ISO, snapshot after first boot | Lima has no Windows guests; Windows containers and Windows Sandbox need a Windows host |

`--keep` stops the VM on exit instead of deleting it and the next run reuses
it — re-running install.sh on it is the idempotency test. One session per OS
at a time: a run refuses a VM that is already `Running`, and a base still
being built is named `…-building` until it is provisioned, so a concurrent run
errors instead of cloning a half-restored disk. Unattended runs need
`BREW_TRUST_NEW_TAPS=1` on the host (forwarded to the guest).
`just sandbox-clean` deletes every sandbox VM.

## Playbook: Windows

### Option A — WSL2 (recommended first)

Inside WSL2 the repo is a Linux install: finish the Linux playbook and it
works unchanged. Only Windows-side GUI apps (Zed, VS Code, Claude desktop)
live outside WSL; configure those from Windows paths (`/mnt/c/Users/<u>/AppData/...`)
and never hardcode `<u>` — derive it with `cmd.exe /c echo %USERNAME%` or `wslvar`.

### Option B — native Windows

Decisions to make up front (ask the user; each changes the architecture):

- **Package owner** — proposed: `config/winget/packages.json` (`winget export` format, applied with `winget import`). winget ships with Windows 11; scoop is the alternative for CLI-heavy setups. Pick one; one owner per tool still applies per OS.
- **`[functions.*]` runtime** — generated scripts are bash. Either require Git for Windows' bash and emit `.cmd` shims, or have the generator emit nu scripts on Windows (port the `on_error = "continue"` step runner).
- **Link strategy** — no stow. A nu linker in `shells.nu` (`^cmd /c mklink` or PowerShell `New-Item -ItemType SymbolicLink`) needs Developer Mode or admin; fail with that message, never silently copy.

Steps, once decided:

1. `bootstrap.ps1` (Windows PowerShell 5.1-compatible): install git + nushell via winget, clone to `%USERPROFILE%\dotconfig`, hand off to nu. Keep all logic after bootstrap in nu so it is shared with Unix.
2. Set user env `XDG_CONFIG_HOME=%USERPROFILE%\.config` (`setx`) before first nu launch so nu and nvim read the same layout as Unix.
3. `shells.nu`: guard `^chmod` by OS; add a per-package OS filter (e.g. `zsh` is Unix-only) and per-OS target overrides (zed → `%APPDATA%\Zed`); implement the Windows link backend and dangling sweep with nu builtins (`ls -l` exposes symlink targets) instead of `find`/`readlink`.
4. `doctor`: port the checks to nu (runs on all three OSes) or add a Windows branch; keep one implementation if possible.
5. `.gitattributes`: `* text=auto eol=lf` so bash/nu files keep LF on Windows checkouts.
6. CI: `windows-latest` job running `shells.nu validate`, `generate` to a temp dir, and nu-check on the output.
7. README "Supported Platforms" + this matrix.
