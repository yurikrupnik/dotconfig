# Helper function to resolve compose files with proper precedence
def resolve_compose_files [
  --file (-f): string  # Optional compose file path
] {
  if ($file | is-not-empty) {
    # Use provided file, resolve relative to current working directory
    let abs_path = ($file | path expand)
    if not ($abs_path | path exists) {
      error make { msg: $"Compose file not found: ($abs_path)" }
    }
    [$abs_path]
  } else {
    # Auto-discover compose files in current directory
    let standard_names = ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"]
    let found_files = ($standard_names
      | where { |name| ([$env.PWD, $name] | path join | path exists) }
      | each { |name| ([$env.PWD, $name] | path join) })

    if ($found_files | is-empty) {
      error make { msg: "No compose file found in current directory. Use --file to specify a custom path." }
    }
    $found_files
  }
}

# Docker compose up with optional file specification
# Usage: compose up [--file path/to/compose.yaml] [additional docker compose args...]
def 'main compose up' [
  --file (-f): string  # Custom compose file path
  ...rest              # Additional arguments passed to docker compose
] {
  let files = (resolve_compose_files --file $file)
  let file_args = ($files | reduce --fold [] { |file, acc| $acc ++ ["-f", $file] })
  let args = ($file_args ++ ["up"] ++ $rest)
  print $args
  docker compose ...$args
}

# Docker compose down with optional file specification
# Usage: compose down [--file path/to/compose.yaml] [additional docker compose args...]
def 'main compose down' [
  --file (-f): string  # Custom compose file path
  ...rest              # Additional arguments passed to docker compose
] {
  let files = (resolve_compose_files --file $file)
  let file_args = ($files | reduce --fold [] { |file, acc| $acc ++ ["-f", $file] })
  let args = ($file_args ++ ["down"] ++ $rest)
  docker compose ...$args
}

def 'main docker prune' [] {
    docker system prune -af
    docker image prune -af
}

def 'main kompose' [
  --file (-f): string,
  ...rest
] {
    # ~/projects/playground/manifests/dockers/compose.yaml
    let files = (resolve_compose_files --file $file)
    let file_args = ($files | reduce --fold [] { |file, acc| $acc ++ ["-f", $file] })
    let args = ($file_args ++ $rest)
    kompose convert --file $file ...$args
}
