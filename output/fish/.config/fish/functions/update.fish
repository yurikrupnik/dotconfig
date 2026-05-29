# Generated from config.toml
# Refresh installed packages on this machine

function update
    brew update
    brew bundle --file ~/dotconfig/config/brew/Brewfile
    brew upgrade
    brew cleanup
    rustup update
    cargo liner ship --no-fail-fast
    cargo install-update -a
    bun update --global
    gcloud components update --quiet
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu stow
end
