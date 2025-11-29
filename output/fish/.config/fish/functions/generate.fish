# Generated from config.toml
# Run generate shells commands, supports zsh, fish and nu scripts.

function generate
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate $argv
end
