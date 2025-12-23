# Generated from config.toml
# Update system packages

function update
    brew update
    brew bundle --file ~/dotconfig/brew/Brewfile --upgrade
    brew bundle cleanup --file ~/dotconfig/brew/Brewfile --force
    brew cleanup
    rustup update
    cargo install-update -a
    gcloud components update
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate $argv
end
