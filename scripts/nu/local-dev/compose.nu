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
    # Return only the first file found (by precedence)
    [($found_files | get 0)]
  }
}

# Helper function to build docker compose arguments
def build_docker_compose_args [
  --file (-f): string,   # Custom compose file path
  subcmd: string,        # Subcommand (up, down, etc.)
  ...rest                # Additional arguments
] {
  let files = (resolve_compose_files --file $file)
  let file_args = ($files | reduce --fold [] { |file, acc| $acc ++ ["-f", $file] })
  $file_args ++ [$subcmd] ++ $rest
}

# Helper function to build kompose arguments
def build_kompose_args [
  --file (-f): string,   # Custom compose file path
  ...rest                # Additional arguments
] {
  let files = (resolve_compose_files --file $file)
  let file_args = ($files | reduce --fold [] { |file, acc| $acc ++ ["-f", $file] })
  $file_args ++ ["convert"] ++ $rest
}

# Docker compose up with optional file specification and arbitrary docker compose flags
# Usage: compose up [--file path/to/compose.yaml] [--detach] [additional docker compose args...]
# Examples:
#   compose up --detach
#   compose up --file custom.yml --detach --build --force-recreate
#   compose up --scale web=3 --remove-orphans
def 'main compose up' [
  --file (-f): string   # Custom compose file path
  ...rest               # Additional arguments passed to docker compose
] {
  let args = (build_docker_compose_args --file $file "up" ...$rest)
  print $args
  print $rest
  docker compose ...$args
}

# Docker compose down with optional file specification
# Usage: compose down [--file path/to/compose.yaml] [additional docker compose args...]
def 'main compose down' [
  --file (-f): string  # Custom compose file path
  ...rest              # Additional arguments passed to docker compose
] {
  let args = (build_docker_compose_args --file $file "down" ...$rest)
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
    let args = (build_kompose_args --file $file ...$rest)
    # Extract the first file for the --file flag
    let files = (resolve_compose_files --file $file)
    let first_file = ($files | get 0)
    kompose convert --file $first_file ...$rest
}
