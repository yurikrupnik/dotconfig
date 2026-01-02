#!/usr/bin/env nu

# Platform Orchestration Commands
# Manage the skills and agents platform

use ../../../nu/shared/shared.nu [log]

const PLATFORM_DIR = "scripts/platform"
const DAPR_COMPONENTS = "scripts/platform/agents/config/dapr"

# Initialize the platform
export def "main platform init" [
    --force(-f)  # Reinitialize even if already done
] {
    log info "Initializing platform..."

    # Check prerequisites
    check-prerequisites

    # Create directory structure
    create-directories

    # Initialize registries
    init-registries $force

    # Initialize Dapr components
    init-dapr-components

    log info "Platform initialized successfully!"
    log info ""
    log info "Next steps:"
    log info "  1. Configure OpenRouter: export OPENROUTER_API_KEY=..."
    log info "  2. Start platform: nu platform.nu up --dapr"
    log info "  3. Create a skill: nu skill.nu create my-skill --type typescript"
}

# Start the platform
export def "main platform up" [
    --dapr               # Start with Dapr sidecars
    --redis              # Start Redis for state/pubsub
    --detach(-d)         # Run in background
] {
    log info "Starting platform..."

    if $redis {
        start-redis $detach
    }

    if $dapr {
        start-dapr $detach
    }

    log info "Platform started!"
    log info ""
    log info "Services:"
    if $redis { log info "  - Redis: localhost:6379" }
    if $dapr { log info "  - Dapr Dashboard: http://localhost:8080" }
}

# Stop the platform
export def "main platform down" [
    --all(-a)  # Stop all services including Redis
] {
    log info "Stopping platform..."

    # Stop Dapr
    try {
        dapr stop --all
    } catch {
        log info "Dapr not running"
    }

    if $all {
        # Stop Redis
        try {
            docker stop platform-redis
            docker rm platform-redis
        } catch {
            log info "Redis not running"
        }
    }

    log info "Platform stopped"
}

# Show platform status
export def "main platform status" [
    --json(-j)  # Output as JSON
] {
    let status = {
        dapr: (check-dapr)
        redis: (check-redis)
        skills: (count-skills)
        agents: (count-agents)
        running_agents: (count-running-agents)
    }

    if $json {
        $status | to json
    } else {
        print "Platform Status"
        print "==============="
        print $"Dapr:           ($status.dapr)"
        print $"Redis:          ($status.redis)"
        print $"Skills:         ($status.skills) registered"
        print $"Agents:         ($status.agents) registered"
        print $"Running Agents: ($status.running_agents)"
    }
}

# Health check
export def "main platform health" [] {
    let checks = [
        { name: "Dapr CLI", check: { which dapr | is-not-empty } }
        { name: "Node.js", check: { which node | is-not-empty } }
        { name: "Rust", check: { which cargo | is-not-empty } }
        { name: "Docker", check: { which docker | is-not-empty } }
        { name: "Redis", check: { check-redis } }
        { name: "OpenRouter API Key", check: { $env.OPENROUTER_API_KEY? | is-not-empty } }
    ]

    print "Health Check"
    print "============"

    $checks | each { |c|
        let status = try { do $c.check } catch { false }
        let icon = if $status { "[OK]" } else { "[FAIL]" }
        print $"($icon) ($c.name)"
    }
}

# Deploy platform to target
export def "main platform deploy" [
    target: string  # Deployment target: k8s, gcp, aws, azure
    --namespace(-n): string = "platform"
] {
    log info $"Deploying to: ($target)"

    match $target {
        "k8s" => { deploy-k8s $namespace }
        "gcp" => { deploy-gcp $namespace }
        "aws" => { deploy-aws $namespace }
        "azure" => { deploy-azure $namespace }
        _ => { log error $"Unknown target: ($target)" }
    }
}

# Helper functions

def check-prerequisites [] {
    let required = ["dapr" "node" "cargo" "docker"]

    $required | each { |cmd|
        if (which $cmd | is-empty) {
            log error $"Required command not found: ($cmd)"
            exit 1
        }
    }

    log info "Prerequisites check passed"
}

def create-directories [] {
    let dirs = [
        $"($PLATFORM_DIR)/skills/examples"
        $"($PLATFORM_DIR)/skills/templates"
        $"($PLATFORM_DIR)/agents/config/dapr"
        $"($PLATFORM_DIR)/agents/templates"
        $"($PLATFORM_DIR)/cloud"
    ]

    $dirs | each { |dir|
        if not ($dir | path exists) {
            mkdir $dir
            log info $"Created: ($dir)"
        }
    }
}

def init-registries [force: bool] {
    let skills_registry = $"($PLATFORM_DIR)/skills/registry.json"
    let agents_registry = $"($PLATFORM_DIR)/agents/registry.json"

    if $force or not ($skills_registry | path exists) {
        { skills: [] } | save -f $skills_registry
        log info "Initialized skills registry"
    }

    if $force or not ($agents_registry | path exists) {
        { agents: [] } | save -f $agents_registry
        log info "Initialized agents registry"
    }
}

def init-dapr-components [] {
    # Statestore
    let statestore = $"($DAPR_COMPONENTS)/statestore.yaml"
    if not ($statestore | path exists) {
        "apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: statestore
spec:
  type: state.redis
  version: v1
  metadata:
    - name: redisHost
      value: localhost:6379
    - name: redisPassword
      value: ''
" | save $statestore
    }

    # Pubsub
    let pubsub = $"($DAPR_COMPONENTS)/pubsub.yaml"
    if not ($pubsub | path exists) {
        "apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: events
spec:
  type: pubsub.redis
  version: v1
  metadata:
    - name: redisHost
      value: localhost:6379
    - name: redisPassword
      value: ''
" | save $pubsub
    }

    log info "Dapr components initialized"
}

def start-redis [detach: bool] {
    log info "Starting Redis..."

    let args = if $detach {
        ["-d" "--name" "platform-redis" "-p" "6379:6379" "redis:7-alpine"]
    } else {
        ["--name" "platform-redis" "-p" "6379:6379" "redis:7-alpine"]
    }

    docker run --rm ...$args
}

def start-dapr [detach: bool] {
    log info "Starting Dapr..."

    if $detach {
        dapr dashboard &
    } else {
        dapr dashboard
    }
}

def check-dapr [] {
    try {
        let version = dapr version | lines | first
        $version != ""
    } catch {
        false
    }
}

def check-redis [] {
    try {
        let pong = redis-cli ping
        $pong == "PONG"
    } catch {
        false
    }
}

def count-skills [] {
    let registry = $"($PLATFORM_DIR)/skills/registry.json"
    if ($registry | path exists) {
        (open $registry).skills | length
    } else {
        0
    }
}

def count-agents [] {
    let registry = $"($PLATFORM_DIR)/agents/registry.json"
    if ($registry | path exists) {
        (open $registry).agents | length
    } else {
        0
    }
}

def count-running-agents [] {
    try {
        dapr list | lines | skip 1 | where { |l| $l =~ "agent-" } | length
    } catch {
        0
    }
}

def deploy-k8s [namespace: string] {
    log info $"Deploying to Kubernetes namespace: ($namespace)"

    # Apply Dapr components
    kubectl apply -f $DAPR_COMPONENTS -n $namespace

    # TODO: Generate and apply K8s manifests for skills/agents
    log info "K8s deployment complete"
}

def deploy-gcp [namespace: string] {
    log info "Deploying to GCP..."
    # TODO: Cloud Run deployment
}

def deploy-aws [namespace: string] {
    log info "Deploying to AWS..."
    # TODO: ECS/Fargate deployment
}

def deploy-azure [namespace: string] {
    log info "Deploying to Azure..."
    # TODO: Container Apps deployment
}

# Main entry point
def main [] {
    print "Platform Orchestration"
    print ""
    print "Commands:"
    print "  platform init     - Initialize the platform"
    print "  platform up       - Start the platform"
    print "  platform down     - Stop the platform"
    print "  platform status   - Show platform status"
    print "  platform health   - Run health checks"
    print "  platform deploy   - Deploy to cloud"
}
