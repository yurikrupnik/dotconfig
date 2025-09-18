#!/usr/bin/env nu

source local-dev/index.nu
# use local-dev/index.nu

# Check if a kind cluster already exists
export def cluster-exists [name: string] {
  kind get clusters
  | lines
  | any { |it| $it == $name }
}

# def main [] {
#     ls
#     cluster-exists kind
#     # main shit
# }
