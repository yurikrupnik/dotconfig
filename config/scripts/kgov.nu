#!/usr/bin/env nu
# Browse `toolbelt govern` cluster findings in nu's interactive explore TUI.
#
#   kgov                  # kind-kind context, clusters scope
#   kgov my-ctx           # another kube context (children still followed)
#   kgov -s clusters,mcp  # widen the scope

def main [
    context: string = "kind-kind"     # kube context to start from
    --scope (-s): string = "clusters" # comma-separated toolbelt govern scopes
] {
    ^toolbelt govern -s $scope -c $context --json | from json | get findings | explore
}
