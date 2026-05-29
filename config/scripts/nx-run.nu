#!/usr/bin/env nu

# Run an Nx command on all projects, capped at $cpu (default: all available CPUs).
export def "main" [
  --cpu(-c): string = ""
  --target(-t): string = "build"
  --command(-s): string = "run-many"
] {
  if $command not-in ["run-many", "affected"] {
      error make {msg: "command must be either 'run-many' or 'affected'"}
  }
  let cpus = if ($cpu | is-empty) {
      sys cpu | length
  } else {
      $cpu | into int
  }

  print $cpus
  bun nx run-many -t $target --parallel $"--max-parallel=($cpus)" --prod
}