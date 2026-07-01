#!/usr/bin/env nu

# Local Development Commands
# Docker Compose and container management utilities

use common.nu *
use config.nu *

# Resolve compose file(s) in current directory or by path
def resolve_compose_files [--file (-f): string]: nothing -> list<string> {
    if ($file | is-not-empty) {
        let abs_path = ($file | path expand)
        if not ($abs_path | path exists) {
            error $"Compose file not found: ($abs_path)"
            exit 1
        }
        return [$abs_path]
    }

    let standard_names = ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"]
    let found_files = (
        $standard_names
        | where { |name| [$env.PWD, $name] | path join | path exists }
        | each { |name| [$env.PWD, $name] | path join }
    )

    if ($found_files | is-empty) {
        # Check configured compose file as fallback
        let manifest_compose = (resolve-config).paths.compose_file
        if ($manifest_compose | path exists) {
            return [($manifest_compose | path expand)]
        }
        error "No compose file found. Use --file to specify a custom path."
        exit 1
    }

    [($found_files | get 0)]
}

# Build docker compose args
def build_docker_compose_args [
    --file (-f): string
    subcmd: string
    ...rest
]: nothing -> list<string> {
    let files = (resolve_compose_files --file $file)
    let file_args = ($files | each { |f| ["-f", $f] } | flatten)
    $file_args ++ [$subcmd] ++ $rest
}

# Start docker compose services
export def "devkit dev up" [
    --file (-f): string  # Custom compose file path
    --detach (-d)        # Run in background
    ...rest              # Additional args
] {
    require-bin "docker"

    let args = if $detach {
        (build_docker_compose_args --file $file "up" "-d" ...$rest)
    } else {
        (build_docker_compose_args --file $file "up" ...$rest)
    }

    info "Starting services..."
    docker compose ...$args
}

# Stop docker compose services
export def "devkit dev down" [
    --file (-f): string  # Custom compose file path
    --volumes (-v)       # Remove volumes
    ...rest              # Additional args
] {
    require-bin "docker"

    let args = if $volumes {
        (build_docker_compose_args --file $file "down" "-v" ...$rest)
    } else {
        (build_docker_compose_args --file $file "down" ...$rest)
    }

    info "Stopping services..."
    docker compose ...$args
}

# Show logs
export def "devkit dev logs" [
    --file (-f): string  # Custom compose file path
    --follow             # Follow log output
    service?: string     # Specific service
] {
    require-bin "docker"

    let args = if $follow {
        if ($service | is-not-empty) {
            (build_docker_compose_args --file $file "logs" "-f" $service)
        } else {
            (build_docker_compose_args --file $file "logs" "-f")
        }
    } else {
        if ($service | is-not-empty) {
            (build_docker_compose_args --file $file "logs" $service)
        } else {
            (build_docker_compose_args --file $file "logs")
        }
    }

    docker compose ...$args
}

# Show status
export def "devkit dev ps" [
    --file (-f): string  # Custom compose file path
] {
    require-bin "docker"

    let args = (build_docker_compose_args --file $file "ps")
    docker compose ...$args
}

# Restart services
export def "devkit dev restart" [
    --file (-f): string  # Custom compose file path
    service?: string     # Specific service
] {
    require-bin "docker"

    let args = if ($service | is-not-empty) {
        (build_docker_compose_args --file $file "restart" $service)
    } else {
        (build_docker_compose_args --file $file "restart")
    }

    info "Restarting services..."
    docker compose ...$args
}

# Prune docker resources
export def "devkit dev prune" [
    --all (-a)  # Remove all unused images, not just dangling
] {
    require-bin "docker"

    warn "This will remove unused Docker resources"

    if $all {
        docker system prune -af
        docker volume prune -af
    } else {
        docker system prune -f
        docker volume prune -f
    }

    success "Docker resources cleaned"
}

# Convert compose to Kubernetes manifests
export def "devkit dev kompose" [
    --file (-f): string  # Custom compose file path
    --namespace (-n): string = "default"  # Target namespace
    --stdout             # Output to stdout instead of files
] {
    require-bin "kompose"

    let files = (resolve_compose_files --file $file)
    let first_file = ($files | get 0)

    info $"Converting ($first_file) to Kubernetes manifests..."

    if $stdout {
        kompose convert --file $first_file --namespace $namespace --stdout
    } else {
        kompose convert --file $first_file --namespace $namespace
        success "Manifests generated"
    }
}

# Reset local environment
export def "devkit dev reset" [
    --file (-f): string  # Custom compose file path
] {
    warn "Resetting local environment (removing volumes)..."

    devkit dev down --file $file --volumes
    devkit dev prune
    devkit dev up --file $file --detach

    success "Local environment reset complete"
}
