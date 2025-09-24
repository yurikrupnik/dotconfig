#!/usr/bin/env nu
# kubernetes cluster bootstrapper (aws/gcp/azure/local)

# =========================
# Constants / "Enums"
# =========================
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

# =========================
# Utilities
# =========================
export def log [level: string, msg: string] {
  # Add timestamp + colors
  let ts = (date now | format date "%d-%m-%Y %H:%M:%S")
  let color = match $level {
      "info"  => "cyan_bold"
      "warning"  => "yellow_bold"
      "error" => "red_bold"
      "critical" => "blue_bold"
      "success" => "green_bold"
      "trace" => "purple_bold"
      "debug" => "blue_bold"
      _       => "white"
  }
  print $"(ansi $color)[($ts)][($level)] ($msg)(ansi reset)"
}

# Check if a command exists
export def command-exists [command: string] {
    (which $command | is-not-empty)
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

# Safer temp file creation
# export def _tmpfile [stem: string, ext: string = "yaml"] {
#   let fname = $"($stem)-((random uuid)).($ext)"
#   $nu.temp-path | path join $fname
# }

export def _tmpfile [stem:string] {
  let dir = $env.TMPDIR? | default "/tmp"
  let ts  = (date now | format date "%Y%m%d%H%M%S")
  $"($dir)/($stem)-($ts)-($env.USER).tmp"
}

# Simple idempotency check for Kind clusters
export def cluster-exists [name: string]: nothing -> bool {
  kind get clusters | lines | any {|it| $it == $name}
}

# =========================
# Public commands
# =========================
def "main list-providers" [] {
  log info "🌩️  Available cloud providers:"
  log warning "🌩️  Available cloud providers:"
  log error "🌩️  Available cloud providers:"
  log success "Some text"
  $PROVIDER_VALUES | each {|p| print $"  • ($p)"}
}

# export def run-par [
#   --max-workers: int = 4
#   --ignore-errors        # don’t throw if any fail
#   ...cmds: list<list<string>>
# ] {
#   let jobs = ($cmds
#     | par-each {|c|
#         let res = (^($c.0) ...($c | skip 1) | complete)
#         { cmd: ($c | str join ' '), exit: $res.exit_code, out: $res.stdout, err: $res.stderr }
#       })

#   if (not $ignore_errors) and ($jobs | any {|j| $j.exit != 0 }) {
#     error make { msg: "One or more commands failed", label: { text: "See results table", span: 0 } }
#   }
#   $jobs
# }

# Initialize KCL project (if not already done)
def "main kcl init" [
    --path(-p): string = "kcl"  # Path to KCL project
] {
    # trace-log "kcl init" "started" --data $"path: ($path)"
    main list-providers
    if (which kcl | is-empty) {
        print "not installed kcl"
        if ($env.OS == "Darwin") {
             print "not installed kcl"
               # curl -fsSL https://kcl-lang.io/install.sh | bash
               #    brew install kcl
        }
    } else {
        print "all good"
    }
    if not ($path | path exists) {
        print $"🔧 Creating KCL project at ($path)..."
        mkdir $path
        cd $path
        kcl mod init
        kcl mod add k8s
        cd ..
        #trace-log "kcl init" "completed" --data $"created: ($path)"
        print "✅ KCL project initialized"
    } else {
        print $"✅ KCL project already exists at ($path)"
        #trace-log "kcl init" "skipped" --data "already exists"
    }
}
