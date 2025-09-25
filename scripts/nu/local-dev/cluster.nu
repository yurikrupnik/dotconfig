#!/usr/bin/env nu
# use
#use std log
#use ../
use ../shared/shared.nu *

def cleanup_local [] {
    print "Cleaning up local resources..."
    cleanup-secrets
    rm -rf tmp/secrets/local
}

export def create [name: string] {
    # ls
    # print $"this is valid ($name)"
    if (cluster-exists $name) {
        log info $"Kind cluster '($name)' already exists — skipping creation."
    } else {
        # ^ls
        # print $"this is valid ($name)"
        log info "🏠 Local Kind cluster creation: ($name)"
        let tmp = (_tmpfile $"kind-config-($env.USER)")

        # defer rm -f $tmp

        let kcl_response = kcl run ~/dotconfig/scripts/kcl/manager/be/kind_cluster.k -D workers=1 -D ingress=true -D name=$name | from yaml
        let config = $kcl_response | get items.0
        $config | to yaml | save -f $tmp --force
        kind create cluster --name $name --config $tmp
        if $env.LAST_EXIT_CODE != 0 {
          error make { msg: "Command failed" }
        }
        kubectl cluster-info --context $"kind-($name)"
        ^kubectl wait --for=condition=Ready nodes --all --timeout=180s
        ^kubectl -n kube-system rollout status deploy/coredns --timeout=180s
        ^kubectl cluster-info --context $"kind-($name)"
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
