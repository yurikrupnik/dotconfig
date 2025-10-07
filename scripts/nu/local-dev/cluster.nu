#!/usr/bin/env nu
# use
#use std log
# import std file
#use ../
use ../shared/shared.nu *

def cleanup_local [] {
    print "Cleaning up local resources..."
    cleanup-secrets
    rm -rf tmp/secrets/local
}

# TODO trying to create schema for function input and output validated
# export alias ClusterCreateOptsa = record<name: string, verbose: bool>
# export def-type ClusterCreateOpts = {
#   name: string,
#   verbose?: bool
# }

export def create []: record<name: string, verbose: bool> -> nothing  {
# export def create []: record<name: string verbose: bool> -> nothing  {
    print $"this is valid ($in)"
    let name = $in.name
    let verbose = $in.verbose
    if (cluster-exists $name) {
        log info $"Kind cluster '($name)' already exists — skipping creation."
    } else {
        log info $"🏠 Local Kind cluster creation: ($name)"
        let tmp = (_tmpfile $"kind-config-($env.USER)")

        # let kcl_response = kcl run ~/dotconfig/scripts/kcl/manager/be/kind_cluster.k -D workers=1 -D ingress=true -D name=$name | from yaml
        let kcl_response = (
            kcl run ~/dotconfig/scripts/kcl/stam/main.k -D workers=2 -D ingress=true -D name=$name | from yaml
            # kck with oci works in theory in any cli - but not in nu - fails to parse output
            # (^kcl run --oci oci://docker.io/yurikrupnik/kcl-stam --tag 0.0.4 -D workers=2 -D ingress=true -D name=$name --format yaml | from yaml)
        )
        let config = $kcl_response | get items.0
        $config | to yaml | save -f $tmp --force
        kind create cluster --name $name --config $tmp
        if $env.LAST_EXIT_CODE != 0 {
          error make { msg: "Command failed" }
        }
        ^kubectl cluster-info --context $"kind-($name)"
        ^kubectl wait --for=condition=Ready nodes --all --timeout=180s
        ^kubectl -n kube-system rollout status deploy/coredns --timeout=180s
        ^kubectl cluster-info --context $"kind-($name)"
        rm -f $tmp
    }

}

export def delete_cluster [name: string] {
    # if cluster.nu
    cluster-exists $name
    kind delete cluster --name $name
}

export def cluster-exists [name: string] {
  kind get clusters
  | lines
  | any { |it| $it == $name }
}

export def increment []: int -> int  {
    $in + 1
    #use std/formats *
    #ls | to jsonl
}

def main [] {
    ls
}
