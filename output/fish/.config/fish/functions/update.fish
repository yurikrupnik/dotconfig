# Generated from config.toml
# Update system packages

function update
    brew update
    brew bundle --file ~/dotconfig/brew/Brewfile --upgrade
    rustup update
    gcloud components update
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate $argv
end
