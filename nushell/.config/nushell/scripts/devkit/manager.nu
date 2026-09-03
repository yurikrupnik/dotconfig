#!/usr/bin/env nu

# devkit manager — role-gated dashboard deployed into the local Kind cluster.
#
# The app (Bun server + UI + manifests) is bundled in ./manager/ next to this
# module. Roles are a security enum in manager/roles.ts (admin, operator,
# developer, viewer); each role only receives the panels it is granted.

use common.nu *
use config.nu *

# Directory holding the bundled dashboard app (resolved at parse time).
const MANAGER_DIR = (path self . | path join "manager")

# Role-gated cluster dashboard. Run a subcommand, or `help devkit manager <cmd>`.
export def "devkit manager" [] {
    print "devkit manager — role-gated dashboard in the local Kind cluster"
    print ""
    print "  devkit manager up [-n CLUSTER]     build image, load into Kind, deploy"
    print "  devkit manager open [--port N]     port-forward + open the dashboard"
    print "  devkit manager status              deployment status"
    print "  devkit manager down                remove the dashboard"
    print ""
    print "Roles (security enum in manager/roles.ts):"
    print "  admin      summary, workloads, events, nodes, secrets, rbac"
    print "  operator   summary, workloads, events, nodes"
    print "  developer  summary, workloads, events"
    print "  viewer     summary"
}

# Render k8s.yaml with the configured namespace/image substituted.
def manifests [cfg: record]: nothing -> string {
    open --raw ($MANAGER_DIR | path join "k8s.yaml")
    | str replace --all "{{NAMESPACE}}" $cfg.manager.namespace
    | str replace --all "{{IMAGE}}" $"($cfg.manager.image):($cfg.manager.tag)"
}

# kubectl context for the configured/named Kind cluster. All subcommands pin
# the context: the image is side-loaded into a specific Kind cluster
# (imagePullPolicy: Never), so acting on whatever context is current would break.
def kind-context [cfg: record, name: string]: nothing -> string {
    let cluster = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    $"kind-($cluster)"
}

# Build the dashboard image, side-load it into the Kind cluster, and deploy.
export def "devkit manager up" [
    --name (-n): string    # Kind cluster name (default: config cluster.name)
] {
    require-bin "docker"
    require-bin "kind"
    require-bin "kubectl"

    let cfg = (resolve-config)
    let cluster = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let ctx = $"kind-($cluster)"
    let image = $"($cfg.manager.image):($cfg.manager.tag)"

    if not (cluster-exists $cluster) {
        error $"Kind cluster '($cluster)' does not exist. Run `devkit up` or pass -n."
        exit 1
    }

    info $"Building image ($image)..."
    # Stow deploys this module as per-file symlinks; docker needs the real
    # directory as build context, so canonicalize through the Dockerfile link.
    let build_dir = (($MANAGER_DIR | path join "Dockerfile") | path expand | path dirname)
    docker build -t $image $build_dir

    info $"Loading image into Kind cluster '($cluster)'..."
    kind load docker-image $image --name $cluster

    info "Applying manifests..."
    manifests $cfg | kubectl --context $ctx apply -f -

    # Restart picks up a freshly loaded image on redeploys (imagePullPolicy: Never).
    do { kubectl --context $ctx -n $cfg.manager.namespace rollout restart deploy/manager } | complete
    kubectl --context $ctx -n $cfg.manager.namespace rollout status deploy/manager --timeout=120s

    success "Manager dashboard deployed"
    info $"Open it with: devkit manager open"
}

# Port-forward the dashboard service and open it in the browser (blocks).
export def "devkit manager open" [
    --name (-n): string    # Kind cluster name (default: config cluster.name)
    --port (-p): int = -1  # Local port (default: config manager.port)
] {
    require-bin "kubectl"

    let cfg = (resolve-config)
    let port = (if $port < 0 { $cfg.manager.port } else { $port })
    let ctx = (kind-context $cfg $name)

    info $"Dashboard: http://localhost:($port)  \(Ctrl-C to stop\)"
    if (is-macos) { do { ^open $"http://localhost:($port)" } | complete }
    kubectl --context $ctx -n $cfg.manager.namespace port-forward svc/manager $"($port):80"
}

# Show dashboard deployment status.
export def "devkit manager status" [
    --name (-n): string    # Kind cluster name (default: config cluster.name)
] {
    require-bin "kubectl"

    let cfg = (resolve-config)
    kubectl --context (kind-context $cfg $name) -n $cfg.manager.namespace get deploy,pod,svc -o wide
}

# Remove the dashboard and its RBAC from the cluster.
export def "devkit manager down" [
    --name (-n): string    # Kind cluster name (default: config cluster.name)
] {
    require-bin "kubectl"

    let cfg = (resolve-config)
    manifests $cfg | kubectl --context (kind-context $cfg $name) delete --ignore-not-found -f -
    success "Manager dashboard removed"
}
