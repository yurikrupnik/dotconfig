#!/usr/bin/env bash
# CI checks shared by .github/workflows/ci.yml and the Tekton pipeline
# (manifests/tekton/ci.yaml); one subcommand per CI step. Tools (shellcheck,
# taplo, nu, zsh, gitleaks) must already be on PATH.
#
#   scripts/ci.sh shellcheck|taplo|nu-check                  # lint job
#   scripts/ci.sh validate|generate|check-generated|nu-check-config   # generator job
#   scripts/ci.sh gitleaks                                   # secrets job (needs full history)
#
# generate writes to $CI_GEN_DIR (default $RUNNER_TEMP/gen, else /tmp/gen);
# check-generated and nu-check-config read it back, so all three must share it.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

gen_dir="${CI_GEN_DIR:-${RUNNER_TEMP:-/tmp}/gen}"
shells="scripts/nu/setup-local-machine/shells.nu"

# shellcheck disable=SC2016  # $env.NU_FILE is nushell, not bash
nucheck() { NU_FILE="$1" nu -n -c 'nu-check --debug $env.NU_FILE' >/dev/null; }

# config.nu/env.nu `use`/`source` files under ~ at parse time (tool init
# scripts, generated.nu); on a real machine setup writes them. Empty stubs are
# enough to parse. They land in $HOME, so only on a CI runner (GitHub Actions
# and the Tekton CI image set CI=true) — never over a developer's real init files.
stub_nu_imports() {
    if [ "${CI:-}" != "true" ]; then
        echo "scripts/ci.sh: nu-check/nu-check-config write stubs under \$HOME; run only with CI=true" >&2
        exit 2
    fi
    sed -nE 's|^[[:space:]]*(use\|source)[[:space:]]+~/([^[:space:]]+).*|\2|p' \
        nushell/.config/nushell/*.nu | sort -u | while IFS= read -r p; do
        mkdir -p "$(dirname "$HOME/$p")" && touch "$HOME/$p"
    done
}

cmd_shellcheck() { shellcheck install.sh bootstrap.sh scripts/*.sh; }

cmd_taplo() { taplo check "config/**/*.toml" devkit.toml; }

cmd_nu_check() {
    stub_nu_imports
    local rc=0 f
    while IFS= read -r f; do
        if nucheck "$f"; then
            echo "ok  $f"
        else
            echo "::error file=$f::nu-check failed"
            rc=1
        fi
    done < <(find config/scripts -maxdepth 1 -name '*.nu'; find scripts/nu -name '*.nu'; find nushell/.config/nushell -maxdepth 1 -name '*.nu')
    return "$rc"
}

cmd_validate() { nu "$shells" validate; }

cmd_generate() {
    nu "$shells" generate \
        --zsh-dir "$gen_dir/zsh" \
        --nu-dir "$gen_dir/nu" \
        --bin-dir "$gen_dir/bin"
}

cmd_check_generated() {
    local rc=0 f name
    zsh -n "$gen_dir/zsh/generated.zsh" || { echo "::error::zsh -n generated.zsh failed"; rc=1; }
    nucheck "$gen_dir/nu/generated.nu" || { echo "::error::nu-check generated.nu failed"; rc=1; }
    for f in "$gen_dir"/bin/*; do
        name="$(basename "$f")"
        case "$(head -n1 "$f")" in
            *bash*|*/sh)
                # macOS /bin/bash is stock 3.2 — the scripts must parse there; Linux checks bash 5.
                if /bin/bash -n "$f" && shellcheck "$f"; then echo "ok  $name (bash)"
                else echo "::error::bin/$name failed bash -n/shellcheck"; rc=1; fi
                ;;
            *nu)
                if nucheck "$f"; then echo "ok  $name (nu)"
                else echo "::error::bin/$name failed nu-check"; rc=1; fi
                ;;
            *)
                echo "::error::bin/$name has an unrecognised shebang: $(head -n1 "$f")"
                rc=1
                ;;
        esac
    done
    return "$rc"
}

cmd_nu_check_config() {
    stub_nu_imports
    cp "$gen_dir/nu/generated.nu" "$HOME/.config/nushell/generated.nu"
    nu -n -c 'nu-check --debug nushell/.config/nushell/config.nu'
}

cmd_gitleaks() { gitleaks git --redact --verbose --exit-code 1 .; }

case "${1:-}" in
    shellcheck) cmd_shellcheck ;;
    taplo) cmd_taplo ;;
    nu-check) cmd_nu_check ;;
    validate) cmd_validate ;;
    generate) cmd_generate ;;
    check-generated) cmd_check_generated ;;
    nu-check-config) cmd_nu_check_config ;;
    gitleaks) cmd_gitleaks ;;
    *)
        echo "usage: $0 shellcheck|taplo|nu-check|validate|generate|check-generated|nu-check-config|gitleaks" >&2
        exit 2
        ;;
esac
