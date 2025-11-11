#!/usr/bin/env nu

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

def "main list-providers" [] {
  log info "🌩️  Available cloud providers:"
  $PROVIDER_VALUES | each {|p| print $"  • ($p)"}
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
