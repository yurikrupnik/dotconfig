# dotconfig — outstanding work (repo review 2026-09-26)

Generated from a review of the current working tree (23 files changed,
~2100 insertions, none yet committed) plus untracked new files. Grouped by
area; each item names the concrete file(s) involved.

## devkit.toml — half-reverted config

- [ ] `devkit.toml` has `[[deps]]` for `tekton-pipelines`, `[namespaces]`,
      `[paths]`, and `[database]` all commented out. AGENTS.md documents
      `just ci-tekton` as running a Tekton `PipelineRun` in the devkit Kind
      cluster, and Tekton as "a `devkit.toml` `[[deps]]` row" — with the dep
      commented out, `just ci-tekton` / `devkit cluster deps` will not
      install Tekton. Decide: restore these sections, or update
      `manifests/tekton/ci.yaml` + AGENTS.md if Tekton is meant to be
      installed a different way now.
- [ ] `[manager]` section in `devkit.toml` carries `# WTF is this manager?` /
      `# TODO devkit manager - it fails` (line 121). Investigate why the
      devkit manager fails and either fix it or remove the section (and any
      docs referencing `devkit manager open`) if it's being dropped.
- [ ] `crossplane` and `external-secrets` `[[deps]]` blocks were rewritten
      from pinned versions (`2.3.4`) to floating `2.*` and both re-commented
      out. Confirm this is intentional before merging — floating majors
      inside a commented block is easy to lose track of.

## Tiltfile — stub only

- [ ] `Tiltfile` (untracked) is still the unmodified `tilt` starter
      template — no real resources, just commented-out examples. `devkit.toml`
      already has `[tilt] enabled = true`. Either fill it in with this
      repo's actual local-env resources or don't enable it in devkit.toml
      yet.

## New scripts need documentation / wiring

- [ ] `config/scripts/ghfleet.nu` (untracked, ~700+ lines) is a substantial
      new GitHub-fleet auditing/generation tool (`ghfleet scan|show|validate|add`)
      but isn't mentioned anywhere in `AGENTS.md` or `README.md`, unlike
      `upkg`, `mcp`, and `toolbelt`. Add a "Key commands" entry once it's
      ready, and confirm its cache dir (`$XDG_CACHE_HOME/ghfleet`) is covered
      by `.gitignore` / not something `just doctor` should flag.
  - [ ] It also generates Kyverno policies / KEDA `ScaledObjects` and can open
        PRs or apply to a cluster (`ghfleet add --apply`) — this is a
        write/mutating path outside the repo; make sure it has its own
        review step before first real use (dry-run / `--only` scoping is
        already there, just confirm before wiring into any automation).
- [ ] `manifests/values/external-secrets.yaml` and `manifests/values/crossplane.yaml`
      are untracked and only referenced from the currently-commented-out
      `[[deps]]` blocks in `devkit.toml` — commit them together with the
      devkit.toml decision above, not separately.

## New CI/Tekton pipeline

- [ ] `manifests/dockers/ci.Dockerfile` + `.github/workflows/ci.yml` both
      pin tool versions; per project rule these must be bumped together —
      do a pass now to confirm they currently match (they were written in
      the same change, but there's no automated check enforcing it).
- [ ] `scripts/ci.sh` is the new single source of truth for CI steps
      (called from ci.yml, Tekton, and `verify.nu`) — do a dry run of each
      step (`scripts/ci.sh <step>` for lint/generator/secrets) locally to
      confirm parity with what ci.yml used to run inline.
- [ ] `just ci-tekton` depends on the `tekton-pipelines` devkit dep that is
      currently commented out (see devkit.toml section above) — this
      command will fail as-is; fix depends on that decision first.

## New `dotconfig` skill (portability)

- [ ] `.claude/skills/dotconfig/scripts/platform-audit.nu` and
      `platform-baseline.json` are new and not yet exercised against a real
      Linux or Windows environment — run `just sandbox-linux` and the
      Windows manual-VM path once to validate the baseline data actually
      matches, per the skill's own portability mandate.
- [ ] `.claude/skills/dotconfig/scripts/verify.nu` duplicates/overlaps
      responsibilities with `scripts/nu/setup-local-machine/shells.nu
      validate` and the new `scripts/ci.sh` — confirm there's no drift
      between what each one checks (avoid three sources of truth for "is
      this repo valid").

## Housekeeping

- [ ] `s.c` at repo root (untracked, `sizeof(struct proc_uniqidentifierinfo)`
      probe) looks like a scratch experiment, not part of the build. Move it
      under `scripts/` with a comment on why it exists (likely calltrace's
      macOS proc-info work) or delete it.
- [ ] `scripts/nu/worktree.nu` is new and used by both `sandbox.nu` and
      `ci-tekton.nu` to pack the working tree — no dedicated test/just
      recipe exists for it standalone; consider a `just` smoke-test recipe
      or at least confirm shellcheck/nu-check cover it (should be automatic
      via lefthook, but worth a manual run once).
- [ ] The full pending changeset (`git status`) is currently one large
      uncommitted working tree — per the repo's Conventional Commits /
      one-concern-per-commit rule, split into separate commits before
      pushing: e.g. `feat(ci): tekton pipeline + ci.sh`, `feat(skills):
      dotconfig portability skill`, `feat(scripts): sandbox/worktree/ghfleet`,
      `fix(devkit): ...`, `docs: ...`.
