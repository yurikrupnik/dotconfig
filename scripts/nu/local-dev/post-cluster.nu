#!/usr/bin/env nu

use ../shared/shared.nu *

# Get GitHub authentication info (token + owner/username)
# Returns a record with { token: string, owner: string } or null if auth fails
def get-github-auth [secrets: record] -> record {
    # Priority 1: Use secrets if both GITHUB_TOKEN and GITHUB_OWNER are provided
    if not ($secrets.GITHUB_TOKEN? | is-empty) and not ($secrets.GITHUB_OWNER? | is-empty) {
        log info "Using GITHUB_TOKEN and GITHUB_OWNER from secrets"
        return { token: $secrets.GITHUB_TOKEN, owner: $secrets.GITHUB_OWNER }
    }

    # Priority 2: Try gh CLI (uses currently authenticated user)
    log info "Attempting to get GitHub auth from gh CLI..."

    # Check if gh is authenticated
    let auth_status = (do --ignore-errors { gh auth status 2>&1 } | complete)
    if $auth_status.exit_code != 0 {
        log error "gh CLI is not authenticated. Run 'gh auth login' first."
        return null
    }

    # Get token from current gh auth
    let token_result = (do --ignore-errors { gh auth token } | complete)
    if $token_result.exit_code != 0 or ($token_result.stdout | str trim | is-empty) {
        log error "Failed to get token from gh auth"
        return null
    }
    let token = ($token_result.stdout | str trim)

    # Get current authenticated username
    let user_result = (do --ignore-errors { gh api user --jq '.login' } | complete)
    if $user_result.exit_code != 0 or ($user_result.stdout | str trim | is-empty) {
        log error "Failed to get GitHub username from gh API"
        return null
    }
    let owner = ($user_result.stdout | str trim)

    log info $"Using GitHub auth for user: ($owner)"
    { token: $token, owner: $owner }
}

# Run commands in parallel and collect results
def run-parallel [tasks: list<record<name: string, cmd: closure>>] {
    log info $"Running ($tasks | length) tasks in parallel..."

    let results = $tasks | par-each { |task|
        let start = (date now)
        log info $"⏳ Starting: ($task.name)"

        let result = try {
            do $task.cmd
            { name: $task.name, success: true, error: null, duration: ((date now) - $start) }
        } catch { |err|
            { name: $task.name, success: false, error: $err.msg, duration: ((date now) - $start) }
        }

        if $result.success {
            log info $"✅ Completed: ($task.name) in ($result.duration)"
        } else {
            log error $"❌ Failed: ($task.name) - ($result.error)"
        }

        $result
    }

    $results
}

# Phase 1: Initial parallel setup (Istio + Flux can run together)
export def "setup phase1" [
    secrets: record
    --repository: string = "gitops-v2"      # GitHub repository name for Flux
    --path: string = "clusters/manager-cluster"  # Path in repository for cluster config
] {
    log info "=== Phase 1: Core Infrastructure (Parallel) ==="

    let tasks = [
        {
            name: "Install Istio Ambient"
            cmd: {||
                istioctl install --set profile=ambient --skip-confirmation | complete
            }
        }
        {
            name: "Create dbs namespace and deploy services"
            cmd: {||
                if (do --ignore-errors { kubectl get ns dbs --no-headers -o name | lines | length }) == 0 {
                    kubectl create namespace dbs
                    kompose convert --file ~/private/nx-playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -
                } else {
                    log info "dbs namespace already exists, skipping"
                }
            }
        }
        {
            name: "Bootstrap Flux GitOps"
            cmd: {||
                if (do --ignore-errors { kubectl -n flux-system get deployment source-controller -o name --no-headers | lines | length }) == 0 {
                    let gh_auth = (get-github-auth $secrets)
                    if $gh_auth == null {
                        log error "Failed to get GitHub authentication - cannot bootstrap Flux"
                        error make { msg: "GitHub authentication failed" }
                    }

                    log info $"Bootstrapping Flux for owner: ($gh_auth.owner), repo: ($repository), path: ($path)"
                    with-env { GITHUB_TOKEN: $gh_auth.token } {
                        flux bootstrap github $"--owner=($gh_auth.owner)" $"--repository=($repository)" --branch=main $"--path=($path)" --personal --components-extra image-reflector-controller,image-automation-controller | complete
                    }
                } else {
                    log info "Flux already bootstrapped, skipping"
                }
            }
        }
    ]

    run-parallel $tasks
}

# Phase 2: Create secrets in parallel (after flux has time to create namespaces)
export def "setup phase2-secrets" [secret_dir: string, secrets: record] {
    log info "=== Phase 2: Secrets Setup (Parallel) ==="

    let secret_puller_file = ($secret_dir | path join "secret-puller.json")
    let onepass_sa_file = ($secret_dir | path join "1pass-sa.txt")
    let container_puller_file = ($secret_dir | path join "container-puller.json")
    let init_yaml_file = ($secret_dir | path join "init.yaml")

    # Validate files exist before starting
    let files_to_check = [
        { path: $secret_puller_file, name: "secret-puller.json" }
        { path: $onepass_sa_file, name: "1pass-sa.txt" }
        { path: $container_puller_file, name: "container-puller.json" }
        { path: $init_yaml_file, name: "init.yaml" }
    ]

    for file in $files_to_check {
        if not ($file.path | path exists) {
            log warning $"Secret file not found: ($file.name) at ($file.path)"
        }
    }

    let tasks = [
        {
            name: "Create secret-puller in default namespace"
            cmd: {||
                if (do --ignore-errors { kubectl get secret secret-puller -n default --no-headers -o name | lines | length }) == 0 {
                    if ($secret_puller_file | path exists) {
                        kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n default
                    }
                }
            }
        }
        {
            name: "Create secret-puller in crossplane-system namespace"
            cmd: {||
                if (do --ignore-errors { kubectl get secret secret-puller -n crossplane-system --no-headers -o name | lines | length }) == 0 {
                    if ($secret_puller_file | path exists) {
                        kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n crossplane-system
                    }
                }
            }
        }
        {
            name: "Create apps namespace and secret-puller"
            cmd: {||
                do --ignore-errors { kubectl create ns apps }
                if (do --ignore-errors { kubectl get secret secret-puller -n apps --no-headers -o name | lines | length }) == 0 {
                    if ($secret_puller_file | path exists) {
                        kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n apps
                    }
                }
            }
        }
        {
            name: "Create 1pass-puller secret"
            cmd: {||
                if (do --ignore-errors { kubectl get secret 1pass-puller -n default --no-headers -o name | lines | length }) == 0 {
                    if ($onepass_sa_file | path exists) {
                        kubectl create secret generic 1pass-puller $"--from-file=creds=($onepass_sa_file)" -n default
                    }
                }
            }
        }
        {
            name: "Create GCP docker registry secret"
            cmd: {||
                if (do --ignore-errors { kubectl get secret gpc-docker-registry-secret --no-headers -o name | lines | length }) == 0 {
                    if ($container_puller_file | path exists) {
                        let container_creds = (open $container_puller_file | str trim)
                        kubectl create secret docker-registry gpc-docker-registry-secret --docker-server=europe-central2-docker.pkg.dev --docker-username=_json_key $"--docker-password=($container_creds)" --docker-email=container-puller@sdp-demo-388112.iam.gserviceaccount.com
                    }
                }
            }
        }
        {
            name: "Create GHCR credentials in flux-system"
            cmd: {||
                if (do --ignore-errors { kubectl get secret ghcr-credentials -n flux-system --no-headers -o name | lines | length }) == 0 {
                    if not ($secrets.DOCKER_PASSWORD? | is-empty) {
                        kubectl create secret docker-registry ghcr-credentials --namespace flux-system --docker-server docker.io --docker-username yurikrupnik $"--docker-password=($secrets.DOCKER_PASSWORD)"
                    } else {
                        log warning "DOCKER_PASSWORD not set, skipping ghcr-credentials"
                    }
                }
            }
        }
        {
            name: "Apply init.yaml for iac-secrets"
            cmd: {||
                if (do --ignore-errors { kubectl get secret iac-secrets -n crossplane-system --no-headers -o name | lines | length }) == 0 {
                    if ($init_yaml_file | path exists) {
                        kubectl apply -f $init_yaml_file
                    }
                }
            }
        }
    ]

    run-parallel $tasks
}

# Phase 3: Wait for controllers in parallel
export def "setup phase3-wait" [] {
    log info "=== Phase 3: Wait for Controllers (Parallel) ==="

    let tasks = [
        {
            name: "Wait for Crossplane deployment"
            cmd: {||
                kubectl -n crossplane-system wait deployment crossplane --for=condition=Available --timeout=180s
            }
        }
        {
            name: "Wait for External Secrets webhook"
            cmd: {||
                kubectl -n external-secrets wait deployment external-secrets-webhook --for=condition=Available --timeout=180s
            }
        }
        {
            name: "Wait for External Secrets cert-controller"
            cmd: {||
                kubectl -n external-secrets wait deployment external-secrets-cert-controller --for=condition=Available --timeout=180s
            }
        }
    ]

    run-parallel $tasks
}

# Phase 4: Wait for Crossplane providers
export def "setup phase4-providers" [] {
    log info "=== Phase 4: Wait for Crossplane Providers (Parallel) ==="

    let tasks = [
        {
            name: "Wait for GCP Storage provider installed"
            cmd: {||
                kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Installed --timeout=180s
            }
        }
        {
            name: "Wait for GCP Storage provider healthy"
            cmd: {||
                kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Healthy --timeout=180s
            }
        }
    ]

    run-parallel $tasks
}

# Run all phases with optional flux stabilization wait
export def "setup all" [
    secret_dir: string
    secrets: record
    --flux-wait: duration = 30sec  # Reduced from 1min since we're doing more in parallel
    --repository: string = "gitops-v2"      # GitHub repository name for Flux
    --path: string = "clusters/manager-cluster"  # Path in repository for cluster config
] {
    log info "🚀 Starting parallel post-cluster setup..."
    let total_start = (date now)

    # Phase 1: Core infrastructure (parallel)
    let phase1_results = (setup phase1 $secrets --repository $repository --path $path)

    # Brief wait for Flux to create namespaces (reduced since other things ran in parallel)
    log info $"⏳ Waiting ($flux_wait) for Flux to stabilize..."
    sleep $flux_wait

    # Phase 2: Secrets (parallel)
    let phase2_results = (setup phase2-secrets $secret_dir $secrets)

    # Phase 3: Wait for controllers (parallel)
    let phase3_results = (setup phase3-wait)

    # Phase 4: Wait for providers (parallel)
    let phase4_results = (setup phase4-providers)

    # Summary
    let all_results = ($phase1_results | append $phase2_results | append $phase3_results | append $phase4_results)
    let succeeded = ($all_results | where success | length)
    let failed = ($all_results | where {|r| not $r.success } | length)
    let total_duration = ((date now) - $total_start)

    log info "═══════════════════════════════════════"
    log info $"📊 Setup Summary: ($succeeded) succeeded, ($failed) failed"
    log info $"⏱️  Total time: ($total_duration)"
    log info "═══════════════════════════════════════"

    if $failed > 0 {
        log error "Some tasks failed:"
        $all_results | where {|r| not $r.success } | each { |r|
            log error $"  - ($r.name): ($r.error)"
        }
    }

    $all_results
}
