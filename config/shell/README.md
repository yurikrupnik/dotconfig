# `config/shell/`

Declarative source of truth for shell **aliases**, **environment variables**, and **simple functions**.

`shells.nu generate` reads `config.toml` and writes:

- `output/zsh/.config/zsh/generated.zsh` — aliases + env vars for zsh
- `output/nu/.config/nushell/generated.nu` — aliases + env vars for nu
- `output/bin/.local/bin/<name>` — one bash script per `[functions.X]`, installed on `PATH`

## When to put a function here

Use `[functions.X]` when **all** of these are true:

- It's a sequence of commands run in order
- No flags, no branches, no loops, and no error handling beyond the two modes below (`on_error`)
- 1–15 lines
- Bash is enough

In other words: when you can express the whole thing as a list of one-line strings.

```toml
[functions.dsp]
description = "Prune all docker data: containers, images, networks, volumes"
commands = [
    "docker system prune -af",
    "docker volume prune -af"
]
```

This is emitted as `~/.local/bin/dsp`:

```bash
#!/usr/bin/env bash
# Generated from config.toml — do not edit by hand.
set -euo pipefail
DOTCONFIG_DIR="${DOTCONFIG_DIR:-$HOME/dotconfig}"
# Prune all docker data: containers, images, networks, volumes

docker system prune -af
docker volume prune -af
```

Allowed keys: `description`, `command` (one string) or `commands` (non-empty list of strings; under `on_error = "continue"` an entry may also be `{ parallel = [...] }`, see below), `on_error`. Anything else is rejected by `shells.nu validate`.

Commands that have a justfile recipe (`brew bundle`, `cargo liner ship`) are routed through `just -f …` to keep the source of truth single. See `config/shell/config.toml` for the full live pipeline.

Every shell finds it on `PATH`, regardless of which shell you're typing into.

## Failure handling (`on_error`)

| `on_error` | Emitted script | Use for |
|---|---|---|
| `"abort"` (default) | `set -euo pipefail`, bare commands | Short sequences where a failed step makes the rest meaningless (`csort`) |
| `"continue"` | `set -uo pipefail` + a `step` runner | Long best-effort pipelines where one flaky upstream must not skip the rest (`update`) |

`"continue"` wraps every command in `step <n> '<command>'`, which prints a `==> [n/total]`
banner, `eval`s the command, and on failure records it and moves on. At the end the
script re-prints every failed step and exits 1:

```
==> [5/14] rustup self update
✗ step 5/14 failed (exit 1): rustup self update
…
1 of 14 steps failed:
  rustup self update
```

This exists because `update` is a machine refresher: before it, a transient
`rustup self update` failure aborted the script and silently skipped the cargo,
bun, node, uv, gcloud, and regen steps.

### Parallel lanes (`parallel`)

Under `on_error = "continue"`, a `commands` entry may be `{ parallel = [lane, lane, …] }`:
two or more lanes, each a non-empty list of commands. Lanes run concurrently as
background jobs, each lane's steps in order; the entry before it finishes first and
the entry after it waits for every lane. `update` uses this after `brew bundle`
(which relinks tools the lanes call) and before `regen`:

```toml
commands = [
    "just -f $DOTCONFIG_DIR/justfile brew-install",
    { parallel = [
        ["brew cleanup"],
        ["rustup update --no-self-update", "just -f $DOTCONFIG_DIR/justfile cargo-install"],
        ["gcloud components update --quiet"],
    ] },
    "just -f $DOTCONFIG_DIR/justfile regen",
]
```

Steps keep their `[n/total]` numbers. Each step's output goes to its own log and is
replayed in step order: the earliest unfinished step streams live, finished ones
print at once, so the output reads like a sequential run. Failures are collected
into the same summary. Lane commands get stdin from `/dev/null` and a non-tty
stdout, so nothing interactive (sudo, prompts) belongs in a lane. Ctrl-C kills
every lane's process tree. Only put commands in separate lanes if they share no
binaries, files or locks.

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

Bare names — like `update`, `sort`, `generate` — collide with **nushell builtins**. Once a function lives on `PATH`, nu still resolves those names to the builtin, not your script; the generator emits aliases pointing at a function with `^` (`u = ^update`). Aliases whose own name is a nu builtin (`ls`) are emitted for zsh only. An alias, a function, and a `config/scripts/` file may not share a name — validation rejects it.

## Top-level tables

Only these three; any other table (e.g. a `[function.x]` typo) fails validation.

| Table | Purpose |
|---|---|
| `[aliases]` | One-liner aliases. Emitted as `alias X='Y'` for zsh and `export alias X = Y` for nu. Plain commands only: `'`, `$(`, backticks, `&&`, `\|\|`, `;`, `\|` are rejected — use `[functions.X]` instead. |
| `[functions.X]` | Sequence-of-commands functions. Emitted as bash scripts on `PATH`. See above. |
| `[environment]` | Environment variables exported in every shell. Booleans are written as `true`/`false`; only `$HOME` and `$DOTCONFIG_DIR` may be referenced. |

## After editing

```bash
just regen        # validate, regenerate output/ + restow
```

Or just `u` (the daily refresher, `~/.local/bin/update`) which calls `shells.nu generate` and `stow` at the end.
