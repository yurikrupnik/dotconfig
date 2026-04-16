#!/usr/bin/env bash
set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

# Configuration
DOTCONFIG_DIR="$HOME/dotconfig"
LOGS_DIR="$DOTCONFIG_DIR/logs"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
DATE_SLUG=$(date +"%Y-%m-%d_%H%M%S")
# Parse flags
DRY_RUN=false
REPORT_FORMAT="table"
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        json|yaml|toml|table) REPORT_FORMAT="$arg" ;;
    esac
done

# Detect Kubernetes environment (service account token or KUBERNETES_SERVICE_HOST)
K8S_ENV=false
if [ -f /var/run/secrets/kubernetes.io/serviceaccount/token ] || [ -n "$KUBERNETES_SERVICE_HOST" ]; then
    K8S_ENV=true
fi

mkdir -p "$LOGS_DIR"

# Arrays to collect report data
declare -a STEP_NAMES=()
declare -a STEP_STATUSES=()
declare -a STEP_DURATIONS=()
declare -a STEP_DETAILS=()

# Structured JSON log to stdout (for K8s log collectors: Fluentd, Loki, etc.)
log_json() {
    local level="$1"
    local msg="$2"
    local extra="${3:-}"
    local ts
    ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    if [ -n "$extra" ]; then
        echo "{\"timestamp\":\"$ts\",\"level\":\"$level\",\"message\":\"$msg\",$extra}"
    else
        echo "{\"timestamp\":\"$ts\",\"level\":\"$level\",\"message\":\"$msg\"}"
    fi
}

log_info() {
    if [ "$K8S_ENV" = true ]; then
        log_json "info" "$1"
    else
        echo -e "${GREEN}==>${NC} $1"
    fi
}

log_warn() {
    if [ "$K8S_ENV" = true ]; then
        log_json "warn" "$1" >&2
    else
        echo -e "${YELLOW}==>${NC} $1"
    fi
}

log_error() {
    if [ "$K8S_ENV" = true ]; then
        log_json "error" "$1" >&2
    else
        echo -e "${RED}==>${NC} $1"
    fi
}

# Run a step, time it, and record the result
# Usage: run_step "Step Name" "detail_capture_command" command [args...]
run_step() {
    local name="$1"
    shift

    log_info "$name..."

    local start_time
    start_time=$(date +%s)

    local output
    local status="success"
    if output=$("$@" 2>&1); then
        status="success"
    else
        status="failed"
        log_warn "$name had errors"
    fi

    local end_time
    end_time=$(date +%s)
    local duration=$(( end_time - start_time ))

    STEP_NAMES+=("$name")
    STEP_STATUSES+=("$status")
    STEP_DURATIONS+=("${duration}s")
    STEP_DETAILS+=("$(echo "$output" | tail -5 | tr '\n' ' ' | cut -c1-200)")

    if [ "$K8S_ENV" = true ]; then
        local log_level="info"
        [ "$status" = "failed" ] && log_level="warn"
        log_json "$log_level" "step_completed" \
            "\"step\":\"$name\",\"status\":\"$status\",\"duration_seconds\":$duration"
    else
        if [ "$status" = "success" ]; then
            log_info "$name completed in ${duration}s"
        else
            log_warn "$name completed with errors in ${duration}s"
        fi
    fi
}

# Generate report in the requested format
generate_report() {
    local total_duration="$1"
    local report_file

    case "$REPORT_FORMAT" in
        json)
            report_file="$LOGS_DIR/update_${DATE_SLUG}.json"
            {
                echo "{"
                echo "  \"timestamp\": \"$TIMESTAMP\","
                echo "  \"total_duration\": \"${total_duration}s\","
                echo "  \"steps\": ["
                for i in "${!STEP_NAMES[@]}"; do
                    local comma=","
                    if [ "$i" -eq $(( ${#STEP_NAMES[@]} - 1 )) ]; then
                        comma=""
                    fi
                    # Escape any double quotes in details
                    local escaped_details
                    escaped_details=$(echo "${STEP_DETAILS[$i]}" | sed 's/"/\\"/g')
                    echo "    {"
                    echo "      \"name\": \"${STEP_NAMES[$i]}\","
                    echo "      \"status\": \"${STEP_STATUSES[$i]}\","
                    echo "      \"duration\": \"${STEP_DURATIONS[$i]}\","
                    echo "      \"details\": \"${escaped_details}\""
                    echo "    }${comma}"
                done
                echo "  ]"
                echo "}"
            } > "$report_file"
            ;;
        yaml)
            report_file="$LOGS_DIR/update_${DATE_SLUG}.yaml"
            {
                echo "timestamp: \"$TIMESTAMP\""
                echo "total_duration: \"${total_duration}s\""
                echo "steps:"
                for i in "${!STEP_NAMES[@]}"; do
                    echo "  - name: \"${STEP_NAMES[$i]}\""
                    echo "    status: \"${STEP_STATUSES[$i]}\""
                    echo "    duration: \"${STEP_DURATIONS[$i]}\""
                    echo "    details: \"${STEP_DETAILS[$i]}\""
                done
            } > "$report_file"
            ;;
        toml)
            report_file="$LOGS_DIR/update_${DATE_SLUG}.toml"
            {
                echo "timestamp = \"$TIMESTAMP\""
                echo "total_duration = \"${total_duration}s\""
                echo ""
                for i in "${!STEP_NAMES[@]}"; do
                    echo "[[steps]]"
                    echo "name = \"${STEP_NAMES[$i]}\""
                    echo "status = \"${STEP_STATUSES[$i]}\""
                    echo "duration = \"${STEP_DURATIONS[$i]}\""
                    echo "details = \"${STEP_DETAILS[$i]}\""
                    echo ""
                done
            } > "$report_file"
            ;;
        table|*)
            report_file="$LOGS_DIR/update_${DATE_SLUG}.txt"
            {
                echo "======================================"
                echo "  Update Report - $TIMESTAMP"
                echo "  Total Duration: ${total_duration}s"
                echo "======================================"
                echo ""
                printf "%-35s %-10s %-10s %s\n" "STEP" "STATUS" "DURATION" "DETAILS"
                printf "%-35s %-10s %-10s %s\n" "---" "------" "--------" "-------"
                for i in "${!STEP_NAMES[@]}"; do
                    printf "%-35s %-10s %-10s %s\n" \
                        "${STEP_NAMES[$i]}" \
                        "${STEP_STATUSES[$i]}" \
                        "${STEP_DURATIONS[$i]}" \
                        "$(echo "${STEP_DETAILS[$i]}" | cut -c1-60)"
                done
                echo ""
                echo "Report saved to: $report_file"
            } > "$report_file"
            ;;
    esac

    echo "$report_file"
}

print_table_summary() {
    local total_duration="$1"
    echo ""
    echo -e "${BOLD}======================================"
    echo -e "  Update Report - $TIMESTAMP"
    echo -e "  Total Duration: ${total_duration}s"
    echo -e "======================================${NC}"
    echo ""
    printf "  ${BOLD}%-35s %-10s %-10s${NC}\n" "STEP" "STATUS" "DURATION"
    printf "  %-35s %-10s %-10s\n" "---" "------" "--------"
    for i in "${!STEP_NAMES[@]}"; do
        local color="$GREEN"
        if [ "${STEP_STATUSES[$i]}" = "failed" ]; then
            color="$RED"
        fi
        printf "  %-35s ${color}%-10s${NC} %-10s\n" \
            "${STEP_NAMES[$i]}" \
            "${STEP_STATUSES[$i]}" \
            "${STEP_DURATIONS[$i]}"
    done
    echo ""
}

# ---- Main ----

TOTAL_START=$(date +%s)

cd "$DOTCONFIG_DIR"

if [ "$DRY_RUN" = true ]; then
    log_info "Dry-run mode: simulating update steps"

    run_step "Git pull"              echo "Already up to date."
    run_step "Brew update"           echo "Updated 3 taps"
    run_step "Brew bundle"           echo "Using Brewfile, 42 dependencies satisfied"
    run_step "Brew upgrade"          echo "Upgraded 2 packages: ripgrep 14.1->14.2, fd 10.1->10.2"
    run_step "Brew cleanup"          echo "Removed 5 old versions"
    run_step "Rustup update"         echo "stable-aarch64-apple-darwin updated to 1.82.0"
    run_step "Cargo install: bat"    echo "bat v0.24.0 installed"
    run_step "Cargo install: eza"    echo "eza v0.20.0 installed"
    run_step "Bun global update"     echo "Updated 4 global packages"
    run_step "Gcloud update"         echo "All components up to date"
    run_step "Shell config generate" echo "Generated .bashrc .zshrc .config/fish/config.fish"
    run_step "Shell config stow"     echo "Stowed 3 configs"
    run_step "Build dotconfig CLI"   echo "Compiling dotconfig v0.1.0 (release)"
    # Simulate a failed step to test error reporting
    run_step "Simulated failure"     false
else
    # Update git repository
    run_step "Git pull" git pull

    # Update Homebrew packages
    run_step "Brew update" brew update
    run_step "Brew bundle" brew bundle --file="$DOTCONFIG_DIR/brew/Brewfile"
    run_step "Brew upgrade" brew upgrade
    run_step "Brew cleanup" brew cleanup

    # Update Rust toolchain
    run_step "Rustup update" rustup update

    # Ensure cargo-binstall is available
    if ! command -v cargo-binstall &> /dev/null; then
        run_step "Install cargo-binstall" cargo install cargo-binstall
    fi

    # Update global Cargo packages (skip already-installed)
    if [ -f "$DOTCONFIG_DIR/cargo-install.toml" ]; then
        cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
        while IFS= read -r pkg; do
            bin_name="${pkg}"
            alt_name="${pkg%-cli}"
            if [ -f "$cargo_bin/$bin_name" ] || [ -f "$cargo_bin/$alt_name" ]; then
                log_info "Cargo: $pkg already installed, skipping"
            else
                run_step "Cargo install: $pkg" cargo binstall "$pkg" --no-confirm
            fi
        done < <(grep '^[a-z]' "$DOTCONFIG_DIR/cargo-install.toml")
    fi

    # Update global npm/bun packages
    if command -v bun &> /dev/null; then
        run_step "Bun global update" bun update --global
    elif command -v npm &> /dev/null; then
        run_step "Npm global update" npm update --global
    fi

    # Update cloud tools
    if command -v gcloud &> /dev/null; then
        run_step "Gcloud components update" gcloud components update --quiet
    fi

    # Regenerate shell configurations
    run_step "Shell config generate" bash -c "cd '$DOTCONFIG_DIR/scripts/nu/setup-local-machine' && nu shells.nu generate"
    run_step "Shell config stow" bash -c "cd '$DOTCONFIG_DIR/scripts/nu/setup-local-machine' && nu shells.nu stow"

    # Rebuild dotconfig CLI
    run_step "Build dotconfig CLI" cargo build --release
fi

TOTAL_END=$(date +%s)
TOTAL_DURATION=$(( TOTAL_END - TOTAL_START ))

# Print summary to terminal (or structured JSON to stdout for K8s)
if [ "$K8S_ENV" = true ]; then
    # Emit full structured summary to stdout for K8s log aggregation
    steps_json=""
    for i in "${!STEP_NAMES[@]}"; do
        [ -n "$steps_json" ] && steps_json="$steps_json,"
        steps_json="$steps_json{\"name\":\"${STEP_NAMES[$i]}\",\"status\":\"${STEP_STATUSES[$i]}\",\"duration\":\"${STEP_DURATIONS[$i]}\"}"
    done
    log_json "info" "update_complete" \
        "\"total_duration_seconds\":$TOTAL_DURATION,\"steps\":[$steps_json]"
else
    print_table_summary "$TOTAL_DURATION"
fi

# Save report file (useful even in K8s if volume is mounted)
report_file=$(generate_report "$TOTAL_DURATION")
log_info "Report saved to: $report_file"

# Also append to the running log
LOG_ENTRY="$LOGS_DIR/update_history.log"
echo "[$TIMESTAMP] duration=${TOTAL_DURATION}s format=$REPORT_FORMAT report=$report_file" >> "$LOG_ENTRY"

log_info "Update complete! (${TOTAL_DURATION}s)"
