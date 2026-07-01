# devkit — reusable monorepo dev/ops engine

A [Nushell](https://www.nushell.sh/) module for local cluster + dev/ops tasks,
shared across every monorepo. Logic lives here once; each consuming repo adapts it
with a small `devkit.toml`.

Part of the dotfiles: this dir is the hand-written `nushell` stow package, so it
lands at `~/.config/nushell/scripts/devkit/` — a path Nushell auto-scans
(`NU_LIB_DIRS`). No extra wiring needed after `just regen`.

## Use it

In any Nushell shell:

```nu
use devkit *
devkit up                 # full local environment
devkit cluster create     # just the Kind cluster
devkit dev up -d          # docker compose
devkit secrets fetch      # pull secrets via vals
devkit config             # show effective config for the current repo
```

Cross-shell (bash/zsh/anywhere) via the launcher on PATH (`config/scripts/devkit.sh`
→ `~/.local/bin/devkit`):

```bash
devkit up
devkit cluster create -n dev -w 2
```

## Command surface

| Group | Commands |
|-------|----------|
| lifecycle | `devkit up`, `devkit down`, `devkit status` |
| `cluster` | `create`, `delete`, `list`, `status`, `setup`, `migrate`, `gitops`, `observability`, `local-dev`, `teardown` |
| `dev` | `up`, `down`, `logs`, `ps`, `restart`, `prune`, `kompose`, `reset` |
| `secrets` | `fetch`, `vault`, `load`, `list`, `verify` |
| `setup` | `install`, `build`, `check`, `test`, `vault-setup`, `k8s-setup`, `all` |
| config | `devkit config`, `devkit config --path` |

Cluster/ops commands need the relevant CLIs installed: `kind`, `kubectl`, `tilt`,
`kcl`, `kompose`, `istioctl`, `vals`, `docker`.

## Per-repo config

Copy `devkit.toml.example` to a monorepo root as `devkit.toml` and edit what
differs (paths, namespaces, endpoints, flux repo, db creds…). devkit discovers it
by walking up from `$PWD`; unset keys fall back to the built-in defaults in
[`config.nu`](config.nu). Inspect the merged result with `devkit config`.

## Files

```
mod.nu         entry point: re-exports submodules + up/down/status
config.nu      DEFAULTS + devkit.toml discovery/merge
common.nu      output/log helpers, cluster connectivity
cluster.nu     Kind cluster lifecycle + k8s deploys
local-dev.nu   docker compose wrappers
secrets.nu     vals-based secret fetch/verify
setup.nu       toolchain install, build, check, test
devkit.toml.example   reference config
```
