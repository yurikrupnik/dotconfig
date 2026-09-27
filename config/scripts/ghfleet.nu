#!/usr/bin/env nu
# ghfleet — every GitHub repo you can reach, in one table: metadata, repo
# settings, config files, CI workflows (+ last run), CD resources (Argo CD /
# Flux), Kyverno validations, KEDA scalers and the workloads they target.
# `add` generates the missing Kyverno ValidatingPolicies / KEDA ScaledObjects
# from parameters and opens a PR, or applies them to a cluster. Parameters:
# DEFAULTS below < `-f` values file (same shape) < flags; a `keda.targets`
# record ({name kind namespace min max triggers spec}) overrides all three for
# that workload. Trigger metadata with commas needs the values file.
#
#   ghfleet                                   scan you + your orgs, browse in explore
#   ghfleet scan -o shluviza --missing keda   table / --json
#   ghfleet show owner/repo                   one repo, full record
#   ghfleet validate owner/repo               kyverno apply: policies -> repo manifests
#   ghfleet add owner/repo                    plan only: what would be generated
#   ghfleet add owner/repo -f params.yaml --keda-max 20 --apply        # branch + PR
#   ghfleet add owner/repo --only kyverno --context kind-dev --apply   # cluster
#
# Repos are cached as blob-less shallow clones with a yaml-only sparse
# checkout under $XDG_CACHE_HOME/ghfleet (default ~/.cache/ghfleet), so a
# rescan only fetches new commits. Manifests are read as plain YAML: Helm
# templates and kustomize overlays are not rendered.

const REPO_FIELDS = [
  name nameWithOwner owner description url visibility isArchived isFork isEmpty
  defaultBranchRef primaryLanguage repositoryTopics licenseInfo pushedAt
  stargazerCount deleteBranchOnMerge mergeCommitAllowed squashMergeAllowed
  rebaseMergeAllowed hasIssuesEnabled hasWikiEnabled
]

const SPARSE = ['*.yaml' '*.yml']
const SKIP_PATHS = '(^|/)(node_modules|vendor|dist|target|\.terraform)/'
const WORKFLOW_PATH = '^\.github/workflows/[^/]+\.ya?ml$'
const WORKLOAD_KINDS = [Deployment StatefulSet]
const MANAGED_BY = {"app.kubernetes.io/managed-by": "ghfleet"}

# label -> regex over repo-relative paths (the full tree, not only yaml)
const CONFIG_FILES = {
  dependabot: '^\.github/dependabot\.ya?ml$'
  renovate: '^(\.github/)?(renovate\.json5?|\.renovaterc(\.json)?)$'
  codeowners: '^(\.github/|docs/)?CODEOWNERS$'
  lefthook: '^\.?lefthook\.ya?ml$'
  "pre-commit": '^\.pre-commit-config\.yaml$'
  docker: '(^|/)(Dockerfile|Containerfile)[^/]*$'
  compose: '(^|/)(docker-)?compose\.ya?ml$'
  helm: '(^|/)Chart\.yaml$'
  kustomize: '(^|/)kustomization\.ya?ml$'
  terraform: '\.tf$'
  skaffold: '(^|/)skaffold\.yaml$'
  tilt: '(^|/)Tiltfile$'
  just: '^(justfile|Justfile|\.justfile)$'
  taskfile: '^Taskfile\.ya?ml$'
  nx: '^nx\.json$'
  cargo: '^Cargo\.toml$'
  node: '^package\.json$'
  go: '^go\.mod$'
  python: '^pyproject\.toml$'
  mise: '^\.?mise\.toml$'
  devkit: '^devkit\.toml$'
}

# `add` / `validate` parameters. Precedence: these < --values file < flags.
const DEFAULTS = {
  dir: "deploy"                     # repo dir for generated manifests
  kyverno: {
    policies: [require-labels disallow-latest-tag require-requests-limits disallow-privileged]
    api_version: "policies.kyverno.io/v1"  # Kyverno 1.16 CRDs serve only v1alpha1/v1beta1
    actions: [Audit]                # validationActions: any of Deny, Audit, Warn
    background: true                # evaluation.background.enabled
    labels: ["app.kubernetes.io/name"]
    exclude_namespaces: [kube-system kube-public kyverno flux-system argocd keda]
    custom: []                      # raw policy documents, added verbatim
  }
  keda: {
    targets: []                     # names or {name kind? namespace? min? max? triggers? spec?}; [] = every unscaled workload
    namespace: null                 # null = the manifest's namespace, else omitted (kustomize sets it)
    min: 1
    max: 10
    polling: 30
    cooldown: 300
    triggers: [{type: cpu, metricType: Utilization, metadata: {value: "70"}}]
    spec: {}                        # deep-merged into every ScaledObject spec
  }
}

# ── helpers ──────────────────────────────────────────────────────────────────

def cache-root []: nothing -> string {
  let base = $env.XDG_CACHE_HOME? | default ($nu.home-dir | path join ".cache")
  $base | path join ghfleet
}

def first-line [s: string]: nothing -> string {
  $s | str trim | lines | get -o 0 | default ""
}

def split-csv [s]: nothing -> list<string> {
  if $s == null { [] } else { $s | split row "," | each {|x| $x | str trim } | where $it != "" }
}

# git with gh as the credential helper, so private repos work whatever the
# user's global git config says.
def --wrapped git-gh [dir: string, ...args: string]: nothing -> record {
  ^git -C $dir -c credential.helper= -c "credential.helper=!gh auth git-credential" ...$args | complete
}

def as-list []: any -> list<any> {
  let v = $in
  match ($v | describe | str replace -r '<.*' '') {
    "list" | "table" => $v
    "nothing" => []
    _ => [$v]
  }
}

def is-record []: any -> bool { ($in | describe) starts-with "record" }

def drop-nulls []: record -> record {
  $in | transpose k v | where v != null | reduce -f {} {|r, acc|
    let v = if ($r.v | is-record) { $r.v | drop-nulls } else { $r.v }
    $acc | upsert ([$r.k] | into cell-path) $v
  }
}

# ── discovery ────────────────────────────────────────────────────────────────

def default-owners []: nothing -> list<string> {
  let me = ^gh api user --jq .login | complete
  if $me.exit_code != 0 { error make {msg: $"gh api user failed: (first-line $me.stderr). Run `gh auth login`."} }
  let orgs = ^gh api --paginate user/orgs --jq '.[].login' | complete
  [($me.stdout | str trim)] | append ($orgs.stdout | lines | where $it != "")
}

def meta-of [r: record]: nothing -> record {
  {
    repo: $r.nameWithOwner
    owner: $r.owner.login
    name: $r.name
    url: $r.url
    description: ($r.description? | default "")
    visibility: ($r.visibility | str lowercase)
    archived: $r.isArchived
    fork: $r.isFork
    empty: $r.isEmpty
    branch: ($r.defaultBranchRef?.name? | default "")
    language: ($r.primaryLanguage?.name? | default "")
    topics: ($r.repositoryTopics? | default [] | each {|t| $t.name })
    license: ($r.licenseInfo?.key? | default "")
    pushed: ($r.pushedAt | into datetime)
    stars: $r.stargazerCount
    settings: {
      delete_branch_on_merge: $r.deleteBranchOnMerge
      merge_commit: $r.mergeCommitAllowed
      squash: $r.squashMergeAllowed
      rebase: $r.rebaseMergeAllowed
      issues: $r.hasIssuesEnabled
      wiki: $r.hasWikiEnabled
    }
  }
}

def list-repos [owners: list<string>, archived: bool, forks: bool]: nothing -> list<record> {
  $owners | par-each {|o|
    let r = ^gh repo list $o --limit 5000 --json ($REPO_FIELDS | str join ",") | complete
    if $r.exit_code != 0 { error make {msg: $"gh repo list ($o) failed: (first-line $r.stderr)"} }
    $r.stdout | from json | each {|x| meta-of $x }
  } | flatten | where {|m| ($archived or not $m.archived) and ($forks or not $m.fork) }
}

def view-repo [repo: string]: nothing -> record {
  let r = ^gh repo view $repo --json ($REPO_FIELDS | str join ",") | complete
  if $r.exit_code != 0 { error make {msg: $"gh repo view ($repo) failed: (first-line $r.stderr)"} }
  meta-of ($r.stdout | from json)
}

# Shallow, blob-less clone with a yaml-only sparse checkout; later runs fetch
# the default branch tip and detach onto it.
def sync-clone [m: record, root: string]: nothing -> record {
  let dir = [$root clones $m.owner $m.name] | path join
  if not ($dir | path join .git | path exists) {
    if ($dir | path exists) { rm -rf $dir }
    mkdir ($dir | path dirname)
    let url = $"https://github.com/($m.repo).git"
    let c = git-gh ($dir | path dirname) clone --quiet --depth 1 --filter=blob:none --sparse --branch $m.branch $url $dir
    if $c.exit_code != 0 { return {dir: $dir, sha: "", error: $"clone: (first-line $c.stderr)"} }
    let s = ^git -C $dir sparse-checkout set --no-cone ...$SPARSE | complete
    if $s.exit_code != 0 { return {dir: $dir, sha: "", error: $"sparse-checkout: (first-line $s.stderr)"} }
  } else {
    let f = git-gh $dir fetch --quiet --depth 1 origin $m.branch
    if $f.exit_code != 0 { return {dir: $dir, sha: "", error: $"fetch: (first-line $f.stderr)"} }
    let c = ^git -C $dir checkout --quiet --force --detach FETCH_HEAD | complete
    if $c.exit_code != 0 { return {dir: $dir, sha: "", error: $"checkout: (first-line $c.stderr)"} }
  }
  {dir: $dir, sha: (^git -C $dir rev-parse --short HEAD | str trim), error: null}
}

def gh-extras [repo: string]: nothing -> record {
  let run = ^gh api -X GET $"repos/($repo)/actions/runs" -f per_page=1 --jq '.workflow_runs[0] // empty | {workflow: .name, event, status, conclusion, branch: .head_branch, at: .created_at, url: .html_url}' | complete
  let rules = ^gh api $"repos/($repo)/rulesets" --jq length | complete
  {
    last_run: (if $run.exit_code == 0 and ($run.stdout | str trim) != "" { $run.stdout | from json } else { null })
    rulesets: (if $rules.exit_code == 0 { $rules.stdout | str trim | into int } else { null })
  }
}

# ── manifest parsing ─────────────────────────────────────────────────────────

def category [api: string, kind: string]: nothing -> string {
  let group = if ($api | str contains "/") { $api | split row "/" | first } else { "" }
  if $group == "argoproj.io" { "argocd" } else if ($group | str ends-with "toolkit.fluxcd.io") { "flux" } else if $group == "kyverno.io" or ($group | str ends-with ".kyverno.io") { "kyverno" } else if $group == "keda.sh" { "keda" } else if $group == "apps" and $kind in $WORKLOAD_KINDS { "workload" } else if $kind == "HorizontalPodAutoscaler" { "hpa" } else { "other" }
}

def parse-docs [dir: string, files: list<string>]: nothing -> list<record> {
  $files
  | where {|f| ($f =~ '\.ya?ml$') and ($f !~ $SKIP_PATHS) and ($f !~ $WORKFLOW_PATH) }
  | each {|f|
    let docs = try { open --raw ($dir | path join $f) | from yaml | as-list } catch { [] }
    # kustomization.yaml / components are build config, not cluster objects
    $docs | where {|d| ($d | is-record) and ($d.apiVersion? | describe) == "string" and ($d.kind? | describe) == "string" and not ($d.apiVersion starts-with "kustomize.config.k8s.io/") } | each {|d| {
      file: $f
      api: $d.apiVersion
      kind: $d.kind
      name: ($d.metadata?.name? | default "")
      namespace: ($d.metadata?.namespace? | default null)
      category: (category $d.apiVersion $d.kind)
      doc: $d
    } }
  } | flatten
}

def parse-workflows [dir: string, files: list<string>]: nothing -> list<record> {
  $files | where {|f| $f =~ $WORKFLOW_PATH } | each {|f|
    let w = try { open --raw ($dir | path join $f) | from yaml } catch { null }
    if not ($w | is-record) { {file: $f, name: "", on: [], jobs: 0, error: "unparseable"} } else {
      let on = $w.on? | default ($w.true? | default null)
      {
        file: $f
        name: ($w.name? | default ($f | path basename))
        on: (if ($on | is-record) { $on | columns } else { $on | as-list | each {|x| $x | into string } })
        jobs: ($w.jobs? | default {} | columns | length)
        error: null
      }
    }
  }
}

def kyverno-view [docs: list<record>]: nothing -> record {
  let pols = $docs | where category == kyverno
  let validations = $pols | each {|p|
    let spec = $p.doc.spec? | default {}
    if $p.kind in [ClusterPolicy Policy] {
      $spec.rules? | default [] | where {|r| $r.validate? != null } | each {|r| {
        policy: $p.name
        rule: $r.name
        action: ($r.validate.failureAction? | default ($spec.validationFailureAction? | default "Audit"))
        file: $p.file
      } }
    } else if $p.kind in [ValidatingPolicy NamespacedValidatingPolicy] {
      $spec.validations? | default [] | enumerate | each {|v| {
        policy: $p.name
        rule: ($v.item.messageExpression? | default ($v.item.message? | default $"validation-($v.index)"))
        action: ($spec.validationActions? | default ["Audit"] | str join ",")
        file: $p.file
      } }
    } else { [] }
  } | flatten
  {
    policies: ($pols | each {|p| {kind: $p.kind, name: $p.name, namespace: $p.namespace, file: $p.file} })
    validations: $validations
  }
}

def cd-view [docs: list<record>]: nothing -> record {
  let argo = $docs | where category == argocd | each {|d|
    let s = $d.doc.spec? | default {}
    let src = $s.source? | default ($s.sources? | default [] | get -o 0) | default {}
    {
      kind: $d.kind
      name: $d.name
      repo: ($src.repoURL? | default "")
      path: ($src.path? | default ($src.chart? | default ""))
      destination: ([($s.destination?.server? | default ($s.destination?.name? | default "")) ($s.destination?.namespace? | default "")] | where $it != "" | str join " / ")
      file: $d.file
    }
  }
  let flux = $docs | where category == flux | each {|d|
    let s = $d.doc.spec? | default {}
    {
      kind: $d.kind
      name: $d.name
      namespace: $d.namespace
      source: ($s.sourceRef?.name? | default ($s.chart?.spec?.sourceRef?.name? | default ""))
      path: ($s.path? | default ($s.url? | default ($s.chart?.spec?.chart? | default "")))
      file: $d.file
    }
  }
  let tools = [(if ($argo | is-not-empty) { "argocd" }) (if ($flux | is-not-empty) { "flux" })] | compact
  {tool: (if ($tools | is-empty) { "none" } else { $tools | str join "+" }), argocd: $argo, flux: $flux}
}

def scale-view [docs: list<record>]: nothing -> record {
  let scalers = $docs | where category == keda and kind in [ScaledObject ScaledJob] | each {|d|
    let s = $d.doc.spec? | default {}
    {
      kind: $d.kind
      name: $d.name
      namespace: $d.namespace
      target: ($s.scaleTargetRef?.name? | default "")
      target_kind: ($s.scaleTargetRef?.kind? | default "Deployment")
      min: ($s.minReplicaCount? | default null)
      max: ($s.maxReplicaCount? | default null)
      triggers: ($s.triggers? | default [] | each {|t| $t.type? | default "" })
      file: $d.file
    }
  }
  let hpas = $docs | where category == hpa | each {|d| {
    name: ($d.doc.spec?.scaleTargetRef?.name? | default "")
    kind: ($d.doc.spec?.scaleTargetRef?.kind? | default "")
  } }
  let workloads = $docs | where category == workload | each {|d|
    let keda = $scalers | any {|s| $s.kind == "ScaledObject" and $s.target == $d.name and $s.target_kind == $d.kind }
    let hpa = $hpas | any {|h| $h.name == $d.name and $h.kind == $d.kind }
    {
      kind: $d.kind
      name: $d.name
      namespace: $d.namespace
      scaler: (if $keda { "keda" } else if $hpa { "hpa" } else { null })
      file: $d.file
    }
  }
  {
    scalers: $scalers
    workloads: $workloads
    unscaled: ($workloads | where scaler == null | get name | uniq)
    triggerauths: ($docs | where category == keda and kind in [TriggerAuthentication ClusterTriggerAuthentication] | get name)
  }
}

# Full record for one repo; `docs` holds the parsed manifests (dropped before
# caching, used by add/validate).
def scan-repo [m: record, root: string]: nothing -> record {
  let base = $m | reject empty settings
  let skeleton = {
    config: [], ci: {workflows: [], last_run: null}, cd: {tool: "none", argocd: [], flux: []}
    kyverno: {policies: [], validations: []}, keda: {scalers: [], unscaled: [], triggerauths: []}
    workloads: [], k8s: 0, docs: [], sha: "", clone: ""
  }
  if $m.empty or $m.branch == "" {
    return ($base | merge $skeleton | merge {settings: $m.settings, error: "empty repository"})
  }
  let c = sync-clone $m $root
  let extras = gh-extras $m.repo
  let settings = $m.settings | merge {rulesets: $extras.rulesets}
  if $c.error != null {
    return ($base | merge $skeleton | merge {settings: $settings, ci: {workflows: [], last_run: $extras.last_run}, clone: $c.dir, error: $c.error})
  }
  let files = ^git -C $c.dir ls-tree -r --name-only HEAD | lines
  let docs = parse-docs $c.dir $files
  let scale = scale-view $docs
  $base | merge {
    sha: $c.sha
    clone: $c.dir
    settings: $settings
    config: ($CONFIG_FILES | transpose label re | where {|x| $files | any {|f| $f =~ $x.re } } | get label)
    ci: {workflows: (parse-workflows $c.dir $files), last_run: $extras.last_run}
    cd: (cd-view $docs)
    kyverno: (kyverno-view $docs)
    keda: ($scale | reject workloads)
    workloads: $scale.workloads
    k8s: ($docs | length)
    docs: $docs
    error: null
  }
}

def scan-one [repo: string]: nothing -> record {
  let root = cache-root
  mkdir $root
  let r = scan-repo (view-repo $repo) $root
  if $r.error != null { error make {msg: $"($repo): ($r.error)"} }
  $r
}

def row-of [r: record]: nothing -> record {
  let last = $r.ci.last_run
  {
    repo: $r.repo
    vis: $r.visibility
    lang: $r.language
    pushed: $r.pushed
    workflows: ($r.ci.workflows | length)
    last_run: (if $last == null { "" } else { $last.conclusion? | default $last.status })
    cd: $r.cd.tool
    k8s: $r.k8s
    kyverno: ($r.kyverno.validations | length)
    keda: ($r.keda.scalers | length)
    unscaled: ($r.keda.unscaled | length)
    config: ($r.config | str join " ")
    error: $r.error
  }
}

def missing [r: record, what: string]: nothing -> bool {
  match $what {
    "kyverno" => ($r.k8s > 0 and ($r.kyverno.validations | is-empty))
    "keda" => ($r.keda.unscaled | is-not-empty)
    "cd" => ($r.k8s > 0 and $r.cd.tool == "none")
    "ci" => ($r.ci.workflows | is-empty)
    _ => (error make {msg: $"--missing: expected kyverno|keda|cd|ci, got ($what)"})
  }
}

def load-scan [owner, archived: bool, forks: bool, cached: bool, jobs: int]: nothing -> list<record> {
  let root = cache-root
  let cache = $root | path join scan.json
  let owners = split-csv $owner
  if $cached {
    if not ($cache | path exists) { error make {msg: $"no cached scan at ($cache); run `ghfleet scan` first"} }
    let rows = open $cache | each {|r| $r | update pushed { into datetime } }
    return (if ($owners | is-empty) { $rows } else { $rows | where owner in $owners })
  }
  let owners = if ($owners | is-empty) { default-owners } else { $owners }
  let metas = list-repos $owners $archived $forks
  print -e $"(ansi dark_gray)scanning ($metas | length) repos of ($owners | str join ', ') → ($root)(ansi reset)"
  mkdir $root
  let rows = $metas | par-each --threads $jobs {|m| scan-repo $m $root | reject docs } | sort-by repo
  $rows | to json | save -f $cache
  $rows
}

# ── generation ───────────────────────────────────────────────────────────────

def params [values, flags: record]: nothing -> record {
  let file = if $values == null { {} } else { open $values }
  if not ($file | is-record) { error make {msg: $"($values): expected a record with dir/kyverno/keda keys"} }
  for k in ($file | columns) {
    if $k not-in ($DEFAULTS | columns) { error make {msg: $"($values): unknown key ($k) \(expected: ($DEFAULTS | columns | str join ', ')\)"} }
    if ($DEFAULTS | get $k | is-record) {
      let bad = $file | get $k | columns | where {|c| $c not-in ($DEFAULTS | get $k | columns) }
      if ($bad | is-not-empty) { error make {msg: $"($values): unknown ($k) keys: ($bad | str join ', ') \(expected: ($DEFAULTS | get $k | columns | str join ', ')\)"} }
    }
  }
  let p = $DEFAULTS | merge deep --strategy overwrite $file | merge deep --strategy overwrite ($flags | drop-nulls)
  let bad_actions = $p.kyverno.actions | where {|a| $a not-in [Deny Audit Warn] }
  if ($p.kyverno.actions | is-empty) or ($bad_actions | is-not-empty) { error make {msg: $"kyverno actions must be a non-empty subset of Deny, Audit, Warn, got ($p.kyverno.actions | str join ',')"} }
  if not ($p.kyverno.api_version starts-with "policies.kyverno.io/") { error make {msg: $"kyverno api_version must be policies.kyverno.io/<version>, got ($p.kyverno.api_version)"} }
  if $p.keda.min > $p.keda.max { error make {msg: $"keda min ($p.keda.min) > max ($p.keda.max)"} }
  $p
}

def kv-record [s]: nothing -> record {
  split-csv $s | reduce -f {} {|kv, acc|
    let parts = $kv | split row -n 2 "="
    if ($parts | length) != 2 { error make {msg: $"expected key=value, got ($kv)"} }
    $acc | upsert ([$parts.0] | into cell-path) $parts.1
  }
}

def trigger-of [type, meta]: nothing -> any {
  if $type == null { return null }
  let base = {type: $type, metadata: (kv-record $meta)}
  [(if $type in [cpu memory] { $base | insert metricType Utilization } else { $base })]
}

const ALL_CONTAINERS = "(object.spec.containers + object.spec.?initContainers.orValue([]) + object.spec.?ephemeralContainers.orValue([]))"
const POD_CONTROLLERS = [deployments statefulsets daemonsets replicasets jobs cronjobs]

# Kyverno CEL ValidatingPolicy (policies.kyverno.io/v1) on Pods; autogen
# extends each rule to the pod controllers.
def kyverno-policy [name: string, p: record]: nothing -> record {
  let v = match $name {
    "require-labels" => {
      for l in $p.labels { if ($l =~ "['\\\\]") { error make {msg: $"kyverno label ($l): quotes/backslashes not allowed"} } }
      let names = $p.labels | each {|l| $"'($l)'" } | str join ", "
      {
        # autogen rewrites the literal `object.metadata` prefix to the pod
        # template, so guard with has() instead of `object.?metadata`
        expression: $"has\(object.metadata\) && has\(object.metadata.labels\) && [($names)].all\(l, l in object.metadata.labels && object.metadata.labels[l] != ''\)"
        message: $"Labels required: ($p.labels | str join ', ')."
      }
    }
    "disallow-latest-tag" => {
      # no image = a kustomize patch fragment; the API server rejects real ones
      expression: $"($ALL_CONTAINERS).all\(c, !has\(c.image\) || c.image.contains\('@'\) || \(c.image.split\('/'\)[c.image.split\('/'\).size\(\) - 1].contains\(':'\) && !c.image.endsWith\(':latest'\)\)\)"
      message: "Images must be pinned to a tag other than latest (or a digest)."
    }
    "require-requests-limits" => {
      expression: "object.spec.containers.all(c, has(c.resources) && has(c.resources.requests) && 'cpu' in c.resources.requests && 'memory' in c.resources.requests && has(c.resources.limits) && 'memory' in c.resources.limits)"
      message: "CPU and memory requests and a memory limit are required."
    }
    "disallow-privileged" => {
      expression: $"($ALL_CONTAINERS).all\(c, !c.?securityContext.?privileged.orValue\(false\)\)"
      message: "Privileged containers are not allowed."
    }
    _ => (error make {msg: $"unknown kyverno policy ($name) \(built in: ($DEFAULTS.kyverno.policies | str join ', '); anything else goes in kyverno.custom\)"})
  }
  let constraints = {
    resourceRules: [{apiGroups: [""], apiVersions: [v1], operations: [CREATE UPDATE], resources: [pods]}]
  } | merge (if ($p.exclude_namespaces | is-empty) { {} } else {
    {namespaceSelector: {matchExpressions: [{key: "kubernetes.io/metadata.name", operator: NotIn, values: $p.exclude_namespaces}]}}
  })
  {
    apiVersion: $p.api_version
    kind: ValidatingPolicy
    metadata: {name: $name, labels: $MANAGED_BY}
    spec: {
      validationActions: $p.actions
      evaluation: {background: {enabled: $p.background}}
      matchConstraints: $constraints
      autogen: {podControllers: {controllers: $POD_CONTROLLERS}}
      validations: [$v]
    }
  }
}

def scaled-object [t: record, p: record]: nothing -> record {
  let triggers = $t.triggers? | default $p.triggers | each {|tr|
    $tr | upsert metadata ($tr.metadata? | default {} | items {|k, v| {k: $k, v: ($v | into string)} } | reduce -f {} {|x, acc| $acc | upsert ([$x.k] | into cell-path) $x.v })
  }
  let min = $t.min? | default $p.min
  let max = $t.max? | default $p.max
  if $min > $max { error make {msg: $"($t.name): min ($min) > max ($max)"} }
  if $min == 0 and ($triggers | all {|x| $x.type in [cpu memory] }) {
    error make {msg: $"($t.name): cpu/memory triggers cannot scale to zero; set min >= 1 or add another trigger"}
  }
  let ns = $t.namespace? | default $p.namespace | default $t.manifest_namespace?
  let kind = $t.kind? | default "Deployment"
  let ref = if $kind == "Deployment" { {name: $t.name} } else { {apiVersion: "apps/v1", kind: $kind, name: $t.name} }
  let spec = {
    scaleTargetRef: $ref
    pollingInterval: $p.polling
    cooldownPeriod: $p.cooldown
    minReplicaCount: $min
    maxReplicaCount: $max
    triggers: $triggers
  } | merge deep --strategy overwrite $p.spec | merge deep --strategy overwrite ($t.spec? | default {})
  let meta = {name: $t.name, labels: $MANAGED_BY} | merge (if $ns == null { {} } else { {namespace: $ns} })
  {apiVersion: "keda.sh/v1alpha1", kind: ScaledObject, metadata: $meta, spec: $spec}
}

# {items: [{kind name namespace file doc}], notes: [string]}
def plan-of [r: record, p: record, parts: list<string>, force: bool, explicit_policies: bool]: nothing -> record {
  mut items = []
  mut notes = []
  if "kyverno" in $parts {
    let have = $r.kyverno.policies | get name
    if ($r.kyverno.validations | is-not-empty) and not $force and not $explicit_policies {
      $notes = $notes | append $"kyverno: ($r.kyverno.validations | length) validation rules already in place \(--force or --kyverno-policies to add more\)"
    } else {
      let gen = $p.kyverno.policies | where {|n| $force or $n not-in $have } | each {|n| kyverno-policy $n $p.kyverno }
      let custom = $p.kyverno.custom | where {|d| $force or ($d.metadata?.name? | default "") not-in $have }
      for d in ($gen | append $custom) {
        $items = $items | append {doc: $d, file: ([$p.dir kyverno $"($d.metadata.name).yaml"] | path join)}
      }
      let kept = $p.kyverno.policies | where {|n| not $force and $n in $have }
      if ($kept | is-not-empty) { $notes = $notes | append $"kyverno: already present: ($kept | str join ', ')" }
    }
  }
  if "keda" in $parts {
    let workloads = $r.workloads
    # explicit targets keep their own overrides; manifest namespace is only a fallback
    let targets = if ($p.keda.targets | is-empty) {
      $workloads | where scaler == null | uniq-by kind name | each {|w| {name: $w.name, kind: $w.kind, scaler: null, manifest_namespace: $w.namespace, found: true} }
    } else {
      $p.keda.targets | each {|t|
        let t = if ($t | is-record) { $t } else { {name: $t} }
        let w = $workloads | where name == $t.name | get -o 0
        $t | merge {
          kind: ($t.kind? | default ($w.kind? | default "Deployment"))
          scaler: ($w.scaler? | default null)
          manifest_namespace: ($w.namespace? | default null)
          found: ($w != null or $t.kind? != null)
        }
      }
    }
    if ($targets | is-empty) {
      $notes = $notes | append (if ($workloads | is-empty) { "keda: no Deployment/StatefulSet manifests found; pass --keda-targets" } else { "keda: every workload already has a scaler" })
    }
    for t in $targets {
      if not $t.found {
        $notes = $notes | append $"keda: ($t.name) not found in repo manifests; assuming Deployment"
      }
      if $t.scaler == "keda" and not $force {
        $notes = $notes | append $"keda: ($t.name) already has a ScaledObject \(--force to regenerate\)"
      } else {
        if $t.scaler == "hpa" {
          $notes = $notes | append $"keda: ($t.name) has an HPA; KEDA creates its own, remove the HPA in the same change"
        }
        let d = scaled-object ($t | reject found scaler) $p.keda
        $items = $items | append {doc: $d, file: ([$p.dir keda $"($t.name)-scaledobject.yaml"] | path join)}
      }
    }
  }
  {
    items: ($items | each {|i| {
      kind: $i.doc.kind
      name: $i.doc.metadata.name
      namespace: ($i.doc.metadata.namespace? | default null)
      file: $i.file
      exists: ($r.clone | path join $i.file | path exists)
      doc: $i.doc
    } })
    notes: $notes
  }
}

def docs-yaml [docs: list<record>]: nothing -> string {
  $docs | each {|d| $d | to yaml } | str join "---\n"
}

# Every doc's apiVersion must be served with its kind; discovery gives a clear
# error (missing CRD, or which versions the cluster serves) before apply.
def require-served [context: string, docs: list<record>] {
  for gv in ($docs | each {|d| {api: $d.apiVersion, kind: $d.kind} } | group-by api --to-table) {
    if not ($gv.api | str contains "/") { continue }
    let group = $gv.api | split row "/" | first
    let r = ^kubectl --context $context get --raw $"/apis/($gv.api)" | complete
    if $r.exit_code != 0 {
      let g = ^kubectl --context $context get --raw $"/apis/($group)" | complete
      let served = if $g.exit_code == 0 { $g.stdout | from json | get versions.version | str join ", " } else { "" }
      let hint = if $served == "" { $"($group) is not installed \(install Kyverno/KEDA first\)" } else if $group == "policies.kyverno.io" { $"served: ($served); pass --kyverno-api ($group)/<version>" } else { $"served: ($served)" }
      error make {msg: $"($context): ($gv.api) not served — ($hint)"}
    }
    let kinds = $r.stdout | from json | get resources.kind
    let missing = $gv.items.kind | uniq | where {|k| $k not-in $kinds }
    if ($missing | is-not-empty) { error make {msg: $"($context): ($gv.api) has no ($missing | str join ', ')"} }
  }
}

def apply-cluster [items: list<record>, context: string, dry_run: bool] {
  require-served $context ($items | get doc)
  let args = [--context $context apply --server-side --field-manager ghfleet -f -] | append (if $dry_run { [--dry-run=server] } else { [] })
  let r = docs-yaml ($items | get doc) | ^kubectl ...$args | complete
  print ($r.stdout | str trim)
  if $r.exit_code != 0 { error make {msg: $"kubectl apply failed: (first-line $r.stderr)"} }
}

def open-pr [r: record, items: list<record>, parts: list<string>] {
  let dir = $r.clone
  let base = ^git -C $dir rev-parse HEAD | str trim
  let branch = $"ghfleet/(($parts | str join '-'))-(date now | format date '%Y%m%d%H%M%S')"
  # the clone is a scan cache: always return it to the scanned commit
  let restore = {||
    ^git -C $dir checkout --quiet --force --detach $base | complete
    ^git -C $dir clean -fdq -- ...($items | get file) | complete
    ^git -C $dir branch --quiet -D $branch | complete
  }
  let sw = ^git -C $dir switch --quiet -c $branch | complete
  if $sw.exit_code != 0 { error make {msg: $"git switch: (first-line $sw.stderr)"} }
  for i in $items {
    let path = $dir | path join $i.file
    mkdir ($path | path dirname)
    $i.doc | to yaml | save -f $path
  }
  let what = $parts | str join " and "
  let title = $"feat\(platform\): add ($what) resources"
  let body = ["Generated by `ghfleet add`:" ""] | append ($items | each {|i| $"- `($i.file)` — ($i.kind) ($i.name)" }) | str join "\n"
  let steps = [
    {|| ^git -C $dir add -- ...($items | get file) | complete }
    {|| ^git -C $dir commit --quiet -m $title -m $body | complete }
    {|| git-gh $dir push --quiet origin $"HEAD:refs/heads/($branch)" }
    {|| ^gh pr create -R $r.repo --base $r.branch --head $branch --title $title --body $body | complete }
  ]
  mut out = ""
  for s in $steps {
    let res = do $s
    if $res.exit_code != 0 {
      do $restore
      error make {msg: $"PR step failed: (first-line $res.stderr)"}
    }
    $out = $res.stdout | str trim
  }
  do $restore
  print $"PR: ($out)"
}

def kyverno-flags [policies, action, labels, exclude, api, no_background: bool]: nothing -> record {
  {
    policies: (if $policies == null { null } else { split-csv $policies })
    actions: (if $action == null { null } else { split-csv $action })
    labels: (if $labels == null { null } else { split-csv $labels })
    exclude_namespaces: (if $exclude == null { null } else { split-csv $exclude })
    api_version: $api
    background: (if $no_background { false } else { null })
  }
}

# ── commands ─────────────────────────────────────────────────────────────────

# Scan every repo and browse it in nu's explore TUI (`:q` or Esc to leave).
def main [
  --owner (-o): string     # comma-separated users/orgs (default: you + your orgs)
  --archived               # include archived repos
  --forks                  # include forks
  --cached (-c)            # reuse the last scan instead of syncing
  --jobs (-j): int = 8     # parallel repo syncs
  --json                   # full records as JSON instead of the TUI
] {
  let rows = load-scan $owner $archived $forks $cached $jobs
  if $json { return ($rows | to json) }
  let view = $rows | each {|r| row-of $r | insert detail $r }
  if (is-terminal --stdout) { $view | explore } else { $view | reject detail }
}

# Fleet table (or JSON), optionally filtered to repos missing something.
def "main scan" [
  --owner (-o): string     # comma-separated users/orgs (default: you + your orgs)
  --archived               # include archived repos
  --forks                  # include forks
  --cached (-c)            # reuse the last scan instead of syncing
  --jobs (-j): int = 8     # parallel repo syncs
  --cd: string             # only repos whose CD tool is argocd|flux|argocd+flux|none
  --missing (-m): string   # only repos missing kyverno|keda|cd|ci (k8s repos for kyverno/cd)
  --json                   # full records as JSON
] {
  mut rows = load-scan $owner $archived $forks $cached $jobs
  if $cd != null { $rows = $rows | where {|r| $r.cd.tool == $cd } }
  if $missing != null { $rows = $rows | where {|r| missing $r $missing } }
  if $json { $rows | to json } else { $rows | each {|r| row-of $r } }
}

# One repo, freshly synced: metadata, settings, config, CI, CD, Kyverno, KEDA.
def "main show" [
  repo: string             # owner/name
  --json
] {
  let r = scan-one $repo | reject docs
  if $json { $r | to json } else { $r | table -e }
}

# Run `kyverno apply` of the repo's policies (plus the built-in baseline from
# the same params `add` uses, unless --repo-only) against its manifests.
def "main validate" [
  repo: string                    # owner/name
  --values (-f): path             # params file (yaml/json/nuon/toml): {kyverno: {...}}
  --kyverno-policies: string      # comma-separated built-in policies
  --kyverno-action: string        # comma-separated validationActions: Deny,Audit,Warn
  --kyverno-labels: string        # comma-separated labels require-labels demands
  --kyverno-exclude: string       # comma-separated namespaces the policies skip
  --kyverno-api: string           # ValidatingPolicy apiVersion (default policies.kyverno.io/v1)
  --repo-only                     # only the repo's own policies, no baseline
  --json                          # results as JSON
] {
  let r = scan-one $repo
  let p = params $values {kyverno: (kyverno-flags $kyverno_policies $kyverno_action $kyverno_labels $kyverno_exclude $kyverno_api false)}
  let own = $r.docs | where category == kyverno and kind in [ClusterPolicy Policy ValidatingPolicy NamespacedValidatingPolicy]
  let have = $own | get name
  let baseline = if $repo_only { [] } else {
    $p.kyverno.policies | where {|n| $n not-in $have } | each {|n| kyverno-policy $n $p.kyverno }
    | append ($p.kyverno.custom | where {|d| ($d.metadata?.name? | default "") not-in $have })
  }
  let policies = $own | get doc | append $baseline
  let resources = $r.docs | where {|d| $d.category not-in [kyverno argocd flux keda] and $d.kind != "CustomResourceDefinition" and $d.name != "" }
  if ($policies | is-empty) { error make {msg: $"($repo): no policies \(repo has none and --repo-only was set\)"} }
  if ($resources | is-empty) { print $"($repo): no plain Kubernetes manifests to validate"; return }
  let layers = resource-layers $resources
  let policy_yaml = docs-yaml $policies
  let rows = $layers | par-each {|layer| kyverno-layer $policy_yaml $layer } | flatten | each {|x|
    $x | insert source (if $x.policy in $have { "repo" } else { "baseline" })
  }
  let summary = $rows | group-by result | transpose result n | each {|g| {result: $g.result, n: ($g.n | length)} }
  if $json { return ({repo: $repo, sha: $r.sha, resources: ($resources | length), layers: ($layers | length), summary: $summary, results: $rows} | to json) }
  print $"($repo) @ ($r.sha): ($resources | length) manifests, ($layers | length) kyverno run\(s\): ($summary | each {|s| $'($s.result)=($s.n)' } | str join ' ')"
  $rows | sort-by result policy file
}

# kustomize repos repeat objects (base + overlays, patches); kyverno's CLI
# panics on duplicates, so layer N holds each object's Nth occurrence.
def resource-layers [docs: list<record>]: nothing -> list<list<record>> {
  $docs
  | each {|d| $d | insert key $"($d.api | split row '/' | first)/($d.kind)/($d.namespace | default 'default')/($d.name)" }
  | group-by key --to-table
  | each {|g| $g.items | enumerate | each {|e| $e.item | insert layer $e.index } }
  | flatten
  | group-by layer --to-table
  | each {|g| $g.items }
}

def kyverno-layer [policy_yaml: string, layer: list<record>]: nothing -> list<record> {
  let tmp = mktemp -d -t ghfleet.XXXXXX
  $policy_yaml | save -f ($tmp | path join policies.yaml)
  docs-yaml ($layer | get doc) | save -f ($tmp | path join resources.yaml)
  let res = ^kyverno apply ($tmp | path join policies.yaml) --resource ($tmp | path join resources.yaml) --policy-report --output-format json --remove-color --audit-warn | complete
  rm -rf $tmp
  # mutate policies print "Mutation has been applied successfully." glued to
  # the report and exit 1, so find the report itself rather than trusting the exit code
  let start = $res.stdout | str index-of '{"kind":"ClusterReport"'
  # no matching resources: kyverno prints only a summary line and exits 0
  if $start < 0 and $res.exit_code == 0 { return [] }
  let report = if $start < 0 { null } else { try { $res.stdout | str substring $start.. | lines | first | from json } catch { null } }
  if $report == null {
    let why = $res.stderr + "\n" + $res.stdout | lines | each {|l| $l | str trim } | where {|l| $l != "" and not ($l starts-with "Warning:") } | last 3 | str join " | "
    error make {msg: $"kyverno apply failed \(exit ($res.exit_code)\): ($why)"}
  }
  $report.results? | default [] | each {|x|
    let o = $x.resources? | default [] | get -o 0 | default {}
    let file = $layer | where {|d| $d.kind == $o.kind? and $d.name == $o.name? and ($d.namespace == null or $d.namespace == $o.namespace?) } | get -o 0.file | default ""
    {
      result: $x.result
      policy: $x.policy
      rule: ($x.rule? | default "")
      resource: ([($o.kind? | default "") ($o.namespace? | default "") ($o.name? | default "")] | where $it != "" | str join "/")
      file: $file
      message: ($x.message? | default "")
    }
  }
}

# Generate the Kyverno validations / KEDA ScaledObjects the repo is missing.
# Plan only (prints YAML) unless --apply: commits to a new branch and opens a
# PR, or with --context applies to that cluster (plan = server dry-run).
def "main add" [
  repo: string                    # owner/name
  --only: string                  # kyverno|keda (default: both)
  --values (-f): path             # params file (yaml/json/nuon/toml) — see DEFAULTS in this script
  --dir: string                   # repo dir for generated files (default: deploy)
  --kyverno-policies: string      # comma-separated built-in policies to add
  --kyverno-action: string        # comma-separated validationActions: Deny,Audit,Warn
  --kyverno-labels: string        # comma-separated labels require-labels demands
  --kyverno-exclude: string       # comma-separated namespaces the policies skip
  --kyverno-api: string           # ValidatingPolicy apiVersion (default policies.kyverno.io/v1)
  --kyverno-no-background         # disable background scans
  --keda-targets: string          # comma-separated workloads (default: every unscaled one)
  --keda-namespace: string        # namespace for ScaledObjects
  --keda-min: int                 # minReplicaCount
  --keda-max: int                 # maxReplicaCount
  --keda-polling: int             # pollingInterval seconds
  --keda-cooldown: int            # cooldownPeriod seconds
  --keda-trigger: string          # trigger type (cpu, memory, cron, prometheus, ...); replaces default triggers
  --keda-metadata: string         # trigger metadata as k=v,k=v (e.g. value=80)
  --force                         # generate even when resources already exist
  --context: string               # kube context: apply to the cluster instead of opening a PR
  --apply                         # execute (PR or cluster); default is plan only
  --json                          # plan as JSON (never applies)
] {
  let parts = if $only == null { [kyverno keda] } else { split-csv $only }
  for x in $parts { if $x not-in [kyverno keda] { error make {msg: $"--only: expected kyverno|keda, got ($x)"} } }
  if $keda_metadata != null and $keda_trigger == null { error make {msg: "--keda-metadata needs --keda-trigger"} }
  let r = scan-one $repo
  let file_p = if $values == null { {} } else { open $values }
  let explicit_policies = $kyverno_policies != null or ($file_p.kyverno?.policies? != null)
  let p = params $values {
    dir: $dir
    kyverno: (kyverno-flags $kyverno_policies $kyverno_action $kyverno_labels $kyverno_exclude $kyverno_api $kyverno_no_background)
    keda: {
      targets: (if $keda_targets == null { null } else { split-csv $keda_targets })
      namespace: $keda_namespace
      min: $keda_min
      max: $keda_max
      polling: $keda_polling
      cooldown: $keda_cooldown
      triggers: (trigger-of $keda_trigger $keda_metadata)
    }
  }
  let plan = plan-of $r $p $parts $force $explicit_policies
  if $json { return ({repo: $repo, sha: $r.sha, params: $p, notes: $plan.notes, items: $plan.items} | to json) }
  for n in $plan.notes { print -e $"(ansi yellow)note:(ansi reset) ($n)" }
  if ($plan.items | is-empty) { print $"($repo): nothing to add"; return }
  let dest = if $context != null { $"cluster ($context)" } else { $"PR against ($repo)@($r.branch)" }
  print $"(ansi green_bold)($repo)(ansi reset) @ ($r.sha) → ($dest)"
  print ($plan.items | select kind name namespace file exists | table -i false)
  if not $apply {
    print (docs-yaml ($plan.items | get doc))
    if $context != null { apply-cluster $plan.items $context true }
    print $"(ansi dark_gray)plan only — rerun with --apply to execute(ansi reset)"
    return
  }
  if $context != null { apply-cluster $plan.items $context false } else { open-pr $r $plan.items $parts }
}
