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
devkit                    # overview of the most-used commands
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
| lifecycle | `devkit up` — single generator; extras opt-in: `--istio --core --gitops --observability --flux` (ingress always on). `devkit down [--keep-cluster]`, `devkit status` |
| `cluster` | `create`, `delete`, `list`, `status`, `setup` (`--dbs`/`--istio`/`--flux`/`--external-secrets`), `deps` (install `[[deps]]` helm charts/manifests from devkit.toml), `migrate`, `gitops`, `observability` |
| `dev` | `up`, `down`, `logs`, `ps`, `restart`, `prune`, `kompose`, `reset` |
| `manager` | `up` (build image → `kind load` → deploy), `open` (port-forward + browser), `status`, `down` |
| `fleet` | `hub` (run the fleet hub), `agent` (report this machine; `--once` for cron/smoke), `status`, `open` |
| `secrets` | `fetch`, `vault`, `load`, `list`, `verify` |
| `setup` | `install`, `build`, `check`, `test`, `vault-setup`, `all` |
| config | `devkit config` (expanded view), `devkit config --data` (raw record for piping), `devkit config --path`, `devkit config init` |

### `devkit manager` — role-gated dashboard

A zero-dependency Bun app deployed *into* the Kind cluster (`devkit manager up`,
then `devkit manager open` → http://localhost:8300). It shows cluster info scoped
by a security enum defined once in [`manager/roles.ts`](manager/roles.ts):

| Role | Panels |
|------|--------|
| `admin` | summary, workloads, events, nodes, secrets (metadata only), rbac |
| `operator` | summary, workloads, events, nodes |
| `developer` | summary, workloads, events |
| `viewer` | summary |

The server enforces grants — a role never receives panels outside its set, and
secret *values* are never surfaced. Trust model is local-dev only: the role is
picked in the UI (no authentication); do not expose beyond a local kind cluster.

### `devkit fleet` — self-hosted machine fleet

A zero-dependency Bun app ([`fleet/`](fleet/)) for observing every machine on
your network from one place — desktop, phone, or tablet (the UI is
responsive-first). One always-on machine runs `devkit fleet hub`; every
machine that can run Bun reports with `devkit fleet agent` (push, so sleeping
laptops go *stale* instead of failing scrapes); devices that can't host an
agent — phones, tablets, printers — are probed by the hub itself
(`[[fleet.probes]]`: TCP connect or ICMP ping). History lives in SQLite
(`fleet.db`, retention-swept). Trust model: LAN only — set `fleet.token` to
gate reports; reads are open.

Cluster/ops commands need the relevant CLIs installed: `kind`, `kubectl`, `tilt`,
`kcl`, `kompose`, `istioctl`, `vals`, `docker`.

## Per-repo config

Run `devkit config init` from a monorepo root to scaffold a `devkit.toml` from
the bundled reference (`--force` overwrites; an optional directory arg sets where
it lands, default the git repo root). Then edit what differs (paths, namespaces,
endpoints, flux repo, db creds…); unset keys fall back to the built-in defaults in
[`config.nu`](config.nu). devkit discovers the file by walking up from `$PWD`.
Inspect the merged result with `devkit config`.

## Files

```
mod.nu         entry point: re-exports submodules + up/down/status
config.nu      DEFAULTS + devkit.toml discovery/merge
common.nu      output/log helpers, cluster connectivity
cluster.nu     Kind cluster lifecycle + k8s deploys
local-dev.nu   docker compose wrappers
manager.nu     role-gated dashboard: build/load/deploy into Kind
manager/       bundled dashboard app (Bun server, UI, Dockerfile, manifests)
fleet.nu       machine fleet: hub / agent / status / open
fleet/         bundled fleet app (Bun hub, agent, responsive UI)
secrets.nu     vals-based secret fetch/verify
setup.nu       toolchain install, build, check, test
devkit.toml.example   reference config
```
