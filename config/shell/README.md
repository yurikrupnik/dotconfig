# `config/shell/`

Declarative source of truth for shell **aliases**, **environment variables**, and **simple functions**.

`shells.nu generate` reads `config.toml` and writes:

- `output/zsh/.config/zsh/generated.zsh` — aliases + env vars for zsh
- `output/nu/.config/nushell/generated.nu` — aliases + env vars for nu
- `output/bin/.local/bin/<name>` — one bash script per `[functions.X]`, installed on `PATH`

## When to put a function here

Use `[functions.X]` when **all** of these are true:

- It's a sequence of commands run in order
- No flags, no branches, no loops, no error handling beyond "abort on first failure"
- 1–15 lines
- Bash is enough

In other words: when you can express the whole thing as a list of one-line strings.

```toml
[functions.up]
description = "Refresh installed packages on this machine"
commands = [
    "brew update",
    "brew upgrade",
    "rustup update",
    "just -f $HOME/dotconfig/justfile cargo-install",
    "bun update --global --latest",
]
```

This is emitted as `~/.local/bin/up`:

```bash
#!/usr/bin/env bash
set -euo pipefail
# Refresh installed packages on this machine

brew update
brew upgrade
rustup update
just -f $HOME/dotconfig/justfile cargo-install
bun update --global --latest
```

Commands that have a justfile recipe (`brew bundle`, `cargo liner ship`) are routed through `just -f …` to keep the source of truth single. See `config/shell/config.toml` for the full live pipeline.

Every shell finds it on `PATH`, regardless of which shell you're typing into.

## When **not** to put a function here

If you find yourself wanting any of:

- Command-line flags / argument parsing
- An `if` / `case` / loop
- Reading JSON or TOML
- A nu pipeline with structured data
- More than ~15 lines

…then it belongs in [`config/scripts/`](../scripts/README.md) as a real script file in the language that fits.

Migration from TOML to script is mechanical: copy the `commands` array into a script body, add a shebang, delete the TOML block.

## Name collisions

Bare names — like `update`, `sort`, `generate` — collide with **nushell builtins**. Once a function lives on `PATH`, nu still resolves those names to the builtin, not your script. Pick names that don't shadow builtins (see `help commands` in nu to check).

## Top-level tables

| Table | Purpose |
|---|---|
| `[aliases]` | One-liner aliases. Emitted as `alias X='Y'` for zsh and `export alias X = Y` for nu. Values with `$(…)` or `&&` are auto-promoted to `def` blocks in nu, since nu aliases don't support shell substitution. |
| `[functions.X]` | Sequence-of-commands functions. Emitted as bash scripts on `PATH`. See above. |
| `[environment]` | Environment variables exported in every shell. Booleans are written as `true`/`false`. |

## After editing

```bash
just regen        # regenerate output/ + restow
```

Or just `up` (the daily refresher) which calls `shells.nu generate` and `stow` at the end.
