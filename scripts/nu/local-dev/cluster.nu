#!/usr/bin/env nu
# use
#use std log
#use ../

def cleanup_local [] {
    print "Cleaning up local resources..."
    cleanup-secrets
    rm -rf tmp/secrets/local
}

export def create [name: string] {
    log info "🏠 Local Kind cluster creation"
    if (cluster-exists $name) {
        log warning $"Kind cluster '($name)' already exists — skipping creation."
    } else {
        kind create cluster --name $name
        if $env.LAST_EXIT_CODE != 0 {
          error make { msg: "Command failed" }
        }
        kubectl cluster-info --context $"kind-($name)"
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
