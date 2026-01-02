#!/usr/bin/env nu

# Logging functions
export def "log info" [message: string] {
    print $"(ansi green)INFO:(ansi reset) ($message)"
}

export def "log error" [message: string] {
    print $"(ansi red)ERROR:(ansi reset) ($message)"
}

export def "log warning" [message: string] {
    print $"(ansi yellow)WARNING:(ansi reset) ($message)"
}

export const CLOUD_PROVIDERS = {
  aws: "aws",
  gcp: "gcp",
  local: "local",
  azure: "azure",
}

const PROVIDER_VALUES = [
  $CLOUD_PROVIDERS.aws
  $CLOUD_PROVIDERS.gcp
  $CLOUD_PROVIDERS.local
  $CLOUD_PROVIDERS.azure
]

export def command-exists [command: string]: nothing -> bool {
    which $command | is-not-empty
}

export def _require-bin [name: string] {
  if (which $name | is-empty) {
    error make { msg: $"Required binary not found on PATH: ($name)" }
  }
}

export def _validate-provider [provider: string] {
  if $provider not-in $PROVIDER_VALUES {
    let options = ($PROVIDER_VALUES | str join ", ")
    error make { msg: $"Invalid cloud provider: ($provider). Valid options: ($options)" }
  }
}

export def _tmpfile [stem: string]: nothing -> string {
  let dir = $env.TMPDIR? | default "/tmp"
  let ts = (date now | format date "%Y%m%d%H%M%S")
  $"($dir)/($stem)-($ts)-($env.USER).tmp"
}

export def cluster-exists [name: string]: nothing -> bool {
  kind get clusters | lines | any {|it| $it == $name}
}

# Check current kubernetes context and validate cluster connectivity
# Returns a record with context info and node status
export def check-cluster-connectivity []: nothing -> record<context: string, nodes: list<string>, healthy: bool, error: string> {
  _require-bin "kubectl"

  # Get current context
  let context = try {
    kubectl config current-context | str trim
  } catch {
    return {
      context: ""
      nodes: []
      healthy: false
      error: "No kubernetes context set. Run 'kubectl config use-context <context>' or 'kubectx <context>'"
    }
  }

  if ($context | is-empty) {
    return {
      context: ""
      nodes: []
      healthy: false
      error: "No kubernetes context set"
    }
  }

  # Check nodes
  let nodes_result = try {
    kubectl get nodes -o jsonpath='{.items[*].metadata.name}' | split row ' ' | where {|it| $it | is-not-empty}
  } catch {
    return {
      context: $context
      nodes: []
      healthy: false
      error: $"Failed to get nodes for context '($context)'. Cluster may be unreachable."
    }
  }

  if ($nodes_result | is-empty) {
    return {
      context: $context
      nodes: []
      healthy: false
      error: $"No nodes found in context '($context)'"
    }
  }

  {
    context: $context
    nodes: $nodes_result
    healthy: true
    error: ""
  }
}

# Require a valid kubernetes context with reachable nodes
# Errors if cluster is not accessible
export def require-cluster-connectivity [] {
  let result = check-cluster-connectivity
  if not $result.healthy {
    error make { msg: $result.error }
  }
  log info $"Connected to context '($result.context)' with ($result.nodes | length) node\(s\)"
  $result
}

def "main list-providers" [] {
  log info "🌩️  Available cloud providers:"
  $PROVIDER_VALUES | each {|p| print $"  • ($p)"}
}

# Check all kubernetes contexts and their node status using Rust CLI
# Returns a list of records with context info and node health
export def check-all-contexts [
  --timeout(-t): int = 10  # Timeout in seconds per context
  --use-rust(-r)           # Use Rust CLI (dotconfig) instead of kubectl
]: nothing -> list<record<context: string, nodes: list<string>, healthy: bool, error: string>> {

  if $use_rust {
    # Use Rust CLI for faster concurrent checks
    if (command-exists "dotconfig") {
      let result = try {
        dotconfig cluster check --timeout $timeout --output json | from json
      } catch {
        log error "Failed to run dotconfig cluster check"
        return []
      }
      return $result
    } else {
      log warning "dotconfig binary not found, falling back to kubectl"
    }
  }

  _require-bin "kubectl"

  # Get all context names
  let contexts = try {
    kubectl config get-contexts -o name | lines | where {|it| $it | is-not-empty}
  } catch {
    log error "Failed to get kubectl contexts"
    return []
  }

  if ($contexts | is-empty) {
    log warning "No kubectl contexts found"
    return []
  }

  log info $"Found ($contexts | length) context\(s\). Checking nodes..."

  # Check each context
  $contexts | each {|ctx|
    let nodes_result = try {
      kubectl get nodes --context $ctx -o jsonpath='{.items[*].metadata.name}' --request-timeout $"($timeout)s"
        | split row ' '
        | where {|it| $it | is-not-empty}
    } catch {
      {
        context: $ctx
        nodes: []
        healthy: false
        error: $"Failed to reach cluster or timeout after ($timeout)s"
      }
    }

    if ($nodes_result | describe | str starts-with "record") {
      # Error case - already a record
      $nodes_result
    } else if ($nodes_result | is-empty) {
      {
        context: $ctx
        nodes: []
        healthy: false
        error: "No nodes found"
      }
    } else {
      {
        context: $ctx
        nodes: $nodes_result
        healthy: true
        error: ""
      }
    }
  }
}

# CLI command to check all contexts
def "main check" [
  --use-rust(-r)  # Use Rust CLI for faster concurrent checks
] {
  let results = if $use_rust {
    check-all-contexts --use-rust
  } else {
    check-all-contexts
  }

  # Display results in a table
  print ""
  print "Cluster Health Check Results:"
  print "=============================="

  $results | each {|r|
    let status = if $r.healthy { $"(ansi green)✓(ansi reset)" } else { $"(ansi red)✗(ansi reset)" }
    let node_count = $r.nodes | length
    let info = if $r.healthy {
      $"($node_count) node\(s\)"
    } else {
      $r.error
    }
    print $"($status) ($r.context): ($info)"
  }

  print ""

  # Summary
  let healthy_count = $results | where healthy | length
  let total = $results | length
  if $healthy_count == $total {
    log info $"All ($total) clusters healthy"
  } else {
    log warning $"($healthy_count)/($total) clusters healthy"
  }

  $results
}

def "main kcl init" [
    --path(-p): string = "kcl"
] {
    main list-providers
    if (which kcl | is-empty) {
        log error "kcl not installed"
        if ($env.OS == "Darwin") {
            log info "Install with: brew install kcl"
        }
        return
    }

    if not ($path | path exists) {
        log info $"🔧 Creating KCL project at ($path)..."
        mkdir $path
        cd $path
        kcl mod init
        kcl mod add k8s
        cd ..
        log info "✅ KCL project initialized"
    } else {
        log info $"✅ KCL project already exists at ($path)"
    }
}
