#!/usr/bin/env nu
# Run this repo's CI (lint / generator / secrets jobs of .github/workflows/ci.yml)
# as a Tekton PipelineRun in the local devkit Kind cluster, against the working
# tree (tracked + untracked, non-ignored files — uncommitted changes included).
#
#   nu scripts/nu/ci-tekton.nu            # cluster from devkit.toml [cluster].name
#   nu scripts/nu/ci-tekton.nu -n other   # another Kind cluster
#
# Prereqs: `devkit cluster create` + `devkit cluster deps` (installs Tekton
# Pipelines from devkit.toml [[deps]]), docker, kind, kubectl, tkn.
#
# The CI image (manifests/dockers/ci.Dockerfile) is built from a working-tree
# tarball, tagged by image ID and side-loaded with `kind load`, so nothing is
# pushed to a registry and each distinct tree gets its own tag.

use worktree.nu

const REPO = path self | path dirname | path dirname | path dirname
const NAMESPACE = "dotconfig-ci"
const PIPELINE = "dotconfig-ci"
const IMAGE_REPO = "dotconfig-ci"

def main [
    --name (-n): string  # Kind cluster name (default: devkit.toml cluster.name)
] {
    let cluster = if ($name | is-not-empty) { $name } else {
        open ($REPO | path join "devkit.toml") | get cluster?.name? | default "dev"
    }
    let ctx = $"kind-($cluster)"

    for bin in [docker kind kubectl tkn] {
        if (which $bin | is-empty) { error make --unspanned { msg: $"($bin) not found on PATH" } }
    }
    if $cluster not-in (^kind get clusters | lines) {
        error make --unspanned { msg: $"Kind cluster '($cluster)' not found — run `devkit cluster create`" }
    }
    if (^kubectl --context $ctx get crd pipelineruns.tekton.dev | complete).exit_code != 0 {
        error make --unspanned { msg: $"Tekton Pipelines is not installed in ($ctx) — run `devkit cluster deps`" }
    }
    print "==> Waiting for Tekton controller + webhook"
    ^kubectl --context $ctx -n tekton-pipelines rollout status deploy/tekton-pipelines-controller deploy/tekton-pipelines-webhook --timeout=180s

    print "==> Building CI image from the working tree"
    let tgz = worktree pack $REPO
    let iidfile = mktemp -t dotconfig-ci-iid.XXXXXX
    open --raw $tgz | ^docker build --file manifests/dockers/ci.Dockerfile --iidfile $iidfile -
    rm $tgz
    let id = open --raw $iidfile | str trim | str replace "sha256:" ""
    rm $iidfile
    let image = $"($IMAGE_REPO):($id | str substring 0..11)"
    ^docker tag $id $image

    print $"==> Loading ($image) into Kind cluster ($cluster)"
    ^kind load docker-image $image --name $cluster

    print "==> Applying Tekton Tasks + Pipeline"
    ^kubectl --context $ctx apply -f ($REPO | path join "manifests/tekton/ci.yaml")

    let run = {
        apiVersion: "tekton.dev/v1"
        kind: "PipelineRun"
        metadata: { generateName: $"($PIPELINE)-", namespace: $NAMESPACE }
        spec: {
            pipelineRef: { name: $PIPELINE }
            params: [{ name: "image", value: $image }]
            timeouts: { pipeline: "15m" }
        }
    }
    let run_name = $run | to json | ^kubectl --context $ctx create -f - -o name | str trim | path basename
    print $"==> PipelineRun ($NAMESPACE)/($run_name)"

    # Logs are informational; the verdict comes from the PipelineRun status.
    do -i { ^tkn --context $ctx -n $NAMESPACE pipelinerun logs $run_name --follow }
    ^kubectl --context $ctx -n $NAMESPACE wait $"pipelinerun/($run_name)" --for=jsonpath={.status.completionTime} --timeout=15m | ignore
    let cond = ^kubectl --context $ctx -n $NAMESPACE get pipelinerun $run_name -o json
        | from json
        | get status.conditions
        | where type == "Succeeded"
        | first
    if $cond.status == "True" {
        print $"(ansi green)✓ ($run_name): ($cond.message)(ansi reset)"
    } else {
        print $"(ansi red)✗ ($run_name): ($cond.reason) — ($cond.message)(ansi reset)"
        exit 1
    }
}
