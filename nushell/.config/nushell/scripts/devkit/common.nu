#!/usr/bin/env nu

# Common utilities and helper functions for monorepo scripts

# Output helpers
export def success [msg: string] {
    print $"(ansi green)✓(ansi reset) ($msg)"
}

export def info [msg: string] {
    print $"(ansi blue)ℹ(ansi reset) ($msg)"
}

export def warn [msg: string] {
    print $"(ansi yellow)⚠(ansi reset) ($msg)"
}

export def error [msg: string] {
    print $"(ansi red)✗(ansi reset) ($msg)"
}

# Check if a command exists
export def command-exists [cmd: string]: nothing -> bool {
    (which $cmd | length) > 0
}

# Require a binary to be installed
export def require-bin [cmd: string] {
    if not (command-exists $cmd) {
        error $"Required binary not found: ($cmd)"
        exit 1
    }
}

# Get the monorepo root directory
export def repo-root []: nothing -> string {
    let git_root = (do { git rev-parse --show-toplevel } | complete)
    if $git_root.exit_code == 0 {
        $git_root.stdout | str trim
    } else {
        $env.PWD
    }
}

# Get OS name
export def get-os []: nothing -> string {
    sys host | get name
}

# Check if running on macOS
export def is-macos []: nothing -> bool {
    (get-os) == "Darwin"
}

# Check if running on Linux
export def is-linux []: nothing -> bool {
    (get-os) == "Linux"
}

# Load environment from .env file
export def load-env [
    --env-file: string = ".env"
]: nothing -> record<> {
    if not ($env_file | path exists) {
        return {}
    }

    let lines = (open $env_file | lines)
    mut result = {}

    for line in $lines {
        if ($line | str starts-with "#") or ($line | str trim | is-empty) {
            continue
        }

        if ($line | str contains "=") {
            let parts = ($line | split row "=")
            if ($parts | length) >= 2 {
                let key = ($parts.0 | str trim)
                let value = ($parts | skip 1 | str join "=" | str trim)
                $result = ($result | insert $key $value)
            }
        }
    }

    $result
}

# Run command and capture result
export def run-cmd [cmd: string, ...args]: nothing -> record<stdout: string, stderr: string, exit_code: int> {
    do { ^$cmd ...$args } | complete
}

# Generate temp file path
export def tmpfile [stem: string]: nothing -> string {
    let dir = $env.TMPDIR? | default "/tmp"
    let ts = (date now | format date "%Y%m%d%H%M%S")
    $"($dir)/($stem)-($ts).tmp"
}

# Check if kind cluster exists
export def cluster-exists [name: string]: nothing -> bool {
    kind get clusters | lines | any {|it| $it == $name}
}

# Log functions with namespace support
export def "log info" [message: string] {
    print $"(ansi green)INFO:(ansi reset) ($message)"
}

export def "log error" [message: string] {
    print $"(ansi red)ERROR:(ansi reset) ($message)"
}

export def "log warning" [message: string] {
    print $"(ansi yellow)WARNING:(ansi reset) ($message)"
}

# Cloud provider constants
export const CLOUD_PROVIDERS = {
    aws: "aws"
    gcp: "gcp"
    local: "local"
    azure: "azure"
}

# Check kubernetes cluster connectivity
export def check-cluster-connectivity []: nothing -> record<context: string, nodes: list<string>, healthy: bool, error: string> {
    require-bin "kubectl"

    let context = try {
        kubectl config current-context | str trim
    } catch {
        return {
            context: ""
            nodes: []
            healthy: false
            error: "No kubernetes context set"
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

    let nodes_result = try {
        kubectl get nodes -o jsonpath='{.items[*].metadata.name}' | split row ' ' | where {|it| $it | is-not-empty}
    } catch {
        return {
            context: $context
            nodes: []
            healthy: false
            error: $"Failed to get nodes for context '($context)'"
        }
    }

    let is_healthy = ($nodes_result | length) > 0
    {
        context: $context
        nodes: $nodes_result
        healthy: $is_healthy
        error: ""
    }
}

# Require valid kubernetes connectivity
export def require-cluster-connectivity [] {
    let result = check-cluster-connectivity
    if not $result.healthy {
        error make { msg: $result.error }
    }
    log info $"Connected to context '($result.context)' with ($result.nodes | length) nodes"
    $result
}
