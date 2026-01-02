#!/usr/bin/env nu

# Test Commands - k6 and Kubetest management

use ../shared/shared.nu [log]

const K6_DIR = "tests/k6"
const KUBETEST_DIR = "tests/kubetest"

# ============================================
# k6 Load Tests
# ============================================

# Run all k6 tests
export def "main test k6 all" [
    --scenario(-s): string = "smoke"  # Test scenario: smoke, load, stress, spike, soak
] {
    log info $"Running all k6 tests with scenario: ($scenario)"
    mkdir tests/k6/results

    let tests = [grafana influxdb api full-stack]
    for test in $tests {
        log info $"Running ($test) tests..."
        k6 run --env SCENARIO=($scenario) $"($K6_DIR)/($test).test.js"
    }
}

# Run Grafana load tests
export def "main test k6 grafana" [
    --scenario(-s): string = "smoke"
    --vus(-u): int = 0
    --duration(-d): string = ""
] {
    log info "Running Grafana load tests..."
    run-k6-test "grafana" $scenario $vus $duration
}

# Run InfluxDB load tests
export def "main test k6 influxdb" [
    --scenario(-s): string = "smoke"
    --vus(-u): int = 0
    --duration(-d): string = ""
] {
    log info "Running InfluxDB load tests..."
    run-k6-test "influxdb" $scenario $vus $duration
}

# Run API load tests
export def "main test k6 api" [
    --scenario(-s): string = "smoke"
    --vus(-u): int = 0
    --duration(-d): string = ""
] {
    log info "Running API load tests..."
    run-k6-test "api" $scenario $vus $duration
}

# Run full-stack load tests
export def "main test k6 full-stack" [
    --duration(-d): string = "2m"
] {
    log info "Running full-stack load tests..."
    mkdir tests/k6/results
    k6 run $"($K6_DIR)/full-stack.test.js"
}

def run-k6-test [name: string, scenario: string, vus: int, duration: string] {
    mkdir tests/k6/results

    mut args = [run --env $"SCENARIO=($scenario)"]

    if $vus > 0 {
        $args = ($args | append ["--vus" $"($vus)"])
    }
    if $duration != "" {
        $args = ($args | append ["--duration" $duration])
    }

    $args = ($args | append $"($K6_DIR)/($name).test.js")

    k6 ...$args
}

# ============================================
# Kubetest (Chainsaw) Tests
# ============================================

# Run all kubetest tests
export def "main test kube all" [
    --parallel(-p): int = 4       # Parallel test execution
    --fail-fast(-f)               # Stop on first failure
    --skip-delete(-s)             # Keep resources after test
] {
    log info "Running all Kubernetes tests..."

    mut args = [test --config $"($KUBETEST_DIR)/chainsaw-config.yaml" --test-dir $KUBETEST_DIR]

    if $parallel > 0 {
        $args = ($args | append ["--parallel" $"($parallel)"])
    }
    if $fail_fast {
        $args = ($args | append "--fail-fast")
    }
    if $skip_delete {
        $args = ($args | append "--skip-delete")
    }

    chainsaw ...$args
}

# Run operator tests
export def "main test kube operator" [
    --skip-delete(-s)
] {
    log info "Running operator tests..."
    run-chainsaw-test "operator" $skip_delete
}

# Run deployment tests
export def "main test kube deployment" [
    --skip-delete(-s)
] {
    log info "Running deployment tests..."
    run-chainsaw-test "deployment" $skip_delete
}

# Run Dapr tests
export def "main test kube dapr" [
    --skip-delete(-s)
] {
    log info "Running Dapr tests..."
    run-chainsaw-test "dapr" $skip_delete
}

# Run scaling tests
export def "main test kube scaling" [
    --skip-delete(-s)
] {
    log info "Running scaling tests..."
    run-chainsaw-test "scaling" $skip_delete
}

# Run network tests
export def "main test kube network" [
    --skip-delete(-s)
] {
    log info "Running network tests..."
    run-chainsaw-test "network" $skip_delete
}

def run-chainsaw-test [name: string, skip_delete: bool] {
    mut args = [test --test-dir $"($KUBETEST_DIR)/($name)"]

    if $skip_delete {
        $args = ($args | append "--skip-delete")
    }

    chainsaw ...$args
}

# ============================================
# Test Infrastructure
# ============================================

# Start test infrastructure (compose services)
export def "main test infra up" [] {
    log info "Starting test infrastructure..."
    docker compose -f scripts/nu/local-dev/compose.yaml up -d
    sleep 5sec
    log info "Infrastructure ready"
}

# Stop test infrastructure
export def "main test infra down" [] {
    log info "Stopping test infrastructure..."
    docker compose -f scripts/nu/local-dev/compose.yaml down
}

# Check test infrastructure status
export def "main test infra status" [] {
    docker compose -f scripts/nu/local-dev/compose.yaml ps
}

# ============================================
# Test Reports
# ============================================

# Show k6 test results
export def "main test results k6" [] {
    let results_dir = "tests/k6/results"
    if ($results_dir | path exists) {
        ls $results_dir | select name size modified
    } else {
        log info "No k6 results found. Run tests first."
    }
}

# Show chainsaw test results
export def "main test results kube" [] {
    let report = "chainsaw-report.json"
    if ($report | path exists) {
        open $report | select name passed failed
    } else {
        log info "No chainsaw results found. Run tests first."
    }
}

# ============================================
# Installation
# ============================================

# Install test tools
export def "main test install" [] {
    log info "Installing test tools..."

    # Install k6
    if (which k6 | is-empty) {
        log info "Installing k6..."
        brew install k6
    } else {
        log info "k6 already installed"
    }

    # Install chainsaw
    if (which chainsaw | is-empty) {
        log info "Installing chainsaw..."
        brew install kyverno/tap/chainsaw
    } else {
        log info "chainsaw already installed"
    }

    log info "Test tools installed!"
}

# ============================================
# Quick Test Commands
# ============================================

# Run smoke tests (quick validation)
export def "main test smoke" [] {
    log info "Running smoke tests..."

    # k6 smoke tests
    main test k6 all --scenario smoke

    # Kube tests (if cluster available)
    if (kubectl cluster-info | complete).exit_code == 0 {
        main test kube deployment
    } else {
        log info "Kubernetes cluster not available, skipping kube tests"
    }
}

# Run full test suite
export def "main test full" [] {
    log info "Running full test suite..."

    # Start infrastructure
    main test infra up

    # Wait for services
    sleep 10sec

    # Run k6 load tests
    main test k6 all --scenario load

    # Run kube tests if cluster available
    if (kubectl cluster-info | complete).exit_code == 0 {
        main test kube all
    }

    # Show results
    main test results k6
}

# Main help
def main [] {
    print "Test Commands"
    print ""
    print "k6 Load Tests:"
    print "  test k6 all         - Run all k6 tests"
    print "  test k6 grafana     - Run Grafana load tests"
    print "  test k6 influxdb    - Run InfluxDB load tests"
    print "  test k6 api         - Run API load tests"
    print "  test k6 full-stack  - Run full-stack tests"
    print ""
    print "Kubernetes Tests (Chainsaw):"
    print "  test kube all       - Run all kube tests"
    print "  test kube operator  - Run operator tests"
    print "  test kube deployment - Run deployment tests"
    print "  test kube dapr      - Run Dapr tests"
    print "  test kube scaling   - Run scaling tests"
    print "  test kube network   - Run network tests"
    print ""
    print "Infrastructure:"
    print "  test infra up       - Start test services"
    print "  test infra down     - Stop test services"
    print "  test infra status   - Check service status"
    print ""
    print "Quick Commands:"
    print "  test smoke          - Run quick smoke tests"
    print "  test full           - Run full test suite"
    print "  test install        - Install test tools"
}
