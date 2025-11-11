def write-dotenv [outfile: string = ".env"] {
    let app = ($env.APP_SECRETS | default "{}" | from json)
    let merged = { GITHUB_TOKEN: $env.GITHUB_TOKEN } | merge $app

    $merged
    | transpose key value
    | each {|it|
        let v = if ($it.value | describe | str contains "record") {
            $it.value | to json
        } else {
            $it.value
        }

        let needs_quotes = (
            ($v | str contains "=") or
            ($v | str contains " ") or
            ($v | str contains "\n")
        )

        if $needs_quotes {
            $"($it.key)=\"($v)\""
        } else {
            $"($it.key)=($v)"
        }
    }
    | str join "\n"
    | save --force $outfile
}