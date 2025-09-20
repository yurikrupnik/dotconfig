
#!/usr/bin/env nu

def create [name: string] {
    # if cluster.nu
    kind create cluster
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
