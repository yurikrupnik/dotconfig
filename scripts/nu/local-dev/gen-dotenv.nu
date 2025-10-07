def write-dotenv [outfile: string = ".env"] {
  let app = ($env.APP_SECRETS | default "{}" | from json)
  let merged = { GITHUB_TOKEN: $env.GITHUB_TOKEN } | merge $app

  $merged
  | transpose key value
  | each {|it|
      # Quote values containing spaces/newlines/equals
      let v = if ($it.value | describe | str contains "record") {
        ($it.value | to json)          # if a nested object slips in, keep it JSON
      } else { $it.value }

      if ($v | str contains "=" or $v | str contains " " or $v | str contains "\n") {
        $"($it.key)=\"($v)\""
      } else {
        $"($it.key)=($v)"
      }
    }
  | str join "\n"
  | save --force $outfile
}

# usage:
# teller run -- nu gen-dotenv.nu -c 'write-dotenv ".env"'
# could not able to run successfully