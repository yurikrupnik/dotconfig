# `config/scripts/`

Hand-written scripts — any language. Each file here is copied to `output/bin/.local/bin/<name-without-extension>` by `shells.nu generate` and ends up on `PATH` via stow.

## When to put a script here

When TOML `[functions.X]` isn't enough. Concretely: when any of these are true:

- You want command-line flags / argument parsing
- You need an `if` / `case` / loop
- You're reading structured data (JSON, TOML, YAML)
- You want a nu pipeline with tables / records
- It's longer than ~15 lines of real logic

If none of these apply, prefer [`config/shell/config.toml`](../shell/README.md) — declarative is simpler.

## File naming

| On disk | Installed as |
|---|---|
| `mcp.nu` | `~/.local/bin/mcp` |
| `deploy.sh` | `~/.local/bin/deploy` |
| `cleanup.py` | `~/.local/bin/cleanup` |
| `lint` (no extension) | `~/.local/bin/lint` |

The extension is for **editor support** (syntax highlighting, language server) and is stripped on install. The shebang inside the file determines the actual interpreter — every shell calls the file by its stripped name and the kernel handles the rest.

Dotfiles and `README.md` are skipped.

## Bash vs nu — when to pick which

|  | Bash | Nu |
|---|---|---|
| **Strengths** | Ubiquitous; runs before nu is installed; well-known idioms (`find -exec`, `xargs`); fast process startup | Structured data first-class (JSON/TOML/CSV/tables); typed CLI flags; pipelines that transform records; far more readable for non-trivial logic |
| **Weaknesses** | Quoting hell; clumsy with arrays and structured data; error handling is bolted on | Requires nu installed; smaller community / fewer Stack Overflow answers; version churn (`r#"…"#` vs `r#'…'#`, deprecated config keys) |
| **Pick when** | Bootstrap (must run before nu exists), single-purpose sequences, reaching for a well-known bash idiom | Anything touching JSON/TOML, anything you want to re-read in 6 months, anything with real logic |

Default rule: **nu for new logic-heavy scripts.** Reserve bash for bootstrap (`install.sh`, `bootstrap.sh`, `doctor.sh`) and for the trivial sequence-of-commands case (which `[functions.X]` already emits).

## Example

`mcp.nu` lives here because it reads server definitions, substitutes env vars, and writes JSON files for multiple AI agents — none of which fits in a TOML array of strings.

```nu
#!/usr/bin/env nu

use std log

export def --env "main" [
    --location: list<string> = [".mcp.json"]
    # …
] {
    # logic here
}
```

After editing, run `just regen` to copy the updated source to `output/bin/` (live via the stow symlink).

## After editing

```bash
just regen        # copy updated scripts to output/bin/.local/bin/
```

Or `up` (the daily refresher) which calls `shells.nu generate` and `stow` at the end.

## Name collisions

A few bare names — `update`, `sort`, `generate` — shadow **nushell builtins**. Once a script lives on `PATH`, typing the name in nu still resolves to the builtin, not your script. Pick names that don't collide (use `help commands` in nu to check).
