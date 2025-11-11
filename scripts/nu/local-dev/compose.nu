def resolve_compose_files [--file (-f): string]: nothing -> list<string> {
    if ($file | is-not-empty) {
        let abs_path = ($file | path expand)
        if not ($abs_path | path exists) {
            error make { msg: $"Compose file not found: ($abs_path)" }
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
        error make { msg: "No compose file found in current directory. Use --file to specify a custom path." }
    }

    [($found_files | get 0)]
}

def build_docker_compose_args [
    --file (-f): string
    subcmd: string
    ...rest
]: nothing -> list<string> {
    let files = (resolve_compose_files --file $file)
    let file_args = ($files | each { |f| ["-f", $f] } | flatten)
    $file_args ++ [$subcmd] ++ $rest
}

def build_kompose_args [
    --file (-f): string
    ...rest
]: nothing -> list<string> {
    let files = (resolve_compose_files --file $file)
    let file_args = ($files | each { |f| ["-f", $f] } | flatten)
    $file_args ++ ["convert"] ++ $rest
}

def 'main compose up' [
    --file (-f): string
    ...rest
] {
    let args = (build_docker_compose_args --file $file "up" ...$rest)
    docker compose ...$args
}

def 'main compose down' [
    --file (-f): string
    ...rest
] {
    let args = (build_docker_compose_args --file $file "down" ...$rest)
    docker compose ...$args
}

def 'main docker prune' [] {
    docker system prune -af
    docker image prune -af
    docker volume prune -af
}

def 'main kompose' [
    --file (-f): string
    ...rest
] {
    let files = (resolve_compose_files --file $file)
    let first_file = ($files | get 0)
    kompose convert --file $first_file ...$rest
}
