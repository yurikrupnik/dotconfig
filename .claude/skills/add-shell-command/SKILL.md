---
name: add-shell-command
description: Add or change a shell alias, function, command, or script in dotconfig. Use when the user wants a new CLI shortcut, a command on PATH, or edits to aliases/functions/environment variables.
---

# Adding a shell command

Decision tree (details: `config/shell/README.md`, `config/scripts/README.md`):

1. **Alias** — single command with flags, no quoting tricks →
   `[aliases]` in `config/shell/config.toml`. Constraints: no single quotes in
   the value (generator errors); emitted to both zsh and nushell.
2. **Function** — a SEQUENCE of simple commands, no flags/branches/loops,
   bash sufficient → `[functions.<name>]` in config.toml. Becomes a bash script
   at `~/.local/bin/<name>`, callable from every shell.
3. **Script** — needs arguments, logic, or a better language →
   `config/scripts/<name>.{nu,sh,py}`. Copied to `~/.local/bin/<name>`
   (extension stripped; shebang decides interpreter).

## Naming rules (enforced by pain, not tooling)

- Never `up` — shadows the Upbound CLI (brew upbound/tap/up)
- Bare names that are nushell builtins (`update`, `sort`, `generate`, ...) are
  unreachable by that name in nu — callers need `^name`. The generator emits
  `alias x = ^target` automatically when an alias targets a `[functions.*]`
  name, but scripts in config/scripts get no such help; prefer non-colliding
  names (`help commands` in nu to check).
- Check PATH collisions: `command -v <name>` before choosing.

## Nushell script gotchas (config/scripts/*.nu)

- `$"..."` interpolation executes bare `(word)` as a subexpression — escape
  literal parens: `\(s\)`
- A failing external command aborts the script — wrap expected-failure calls in
  `try { ^cmd } catch { }` or capture with `| complete`
- `math sum` errors on empty input — `| append 0 | math sum`

## Apply and verify

```bash
just regen          # generate output/ + restow (required for config.toml + scripts)
just doctor         # symlink/freshness health check
zsh -ic '<name> ...'            # verify zsh
nu -l -c '<name> ...'           # verify nushell (^-prefix if builtin-shadowed)
```

Open shells keep old aliases until `exec zsh`.
