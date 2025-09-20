source shells.nu

def 'main update' [] {
    brew bundle --file ~/dotconfig/brew/Brewfile
    rustup update
    gcloud components update
    npm update -g
    # nu ~/dotconfig/scripts/nu/index.nu shells generate
    # nu ~/dotfiles/scripts/nu/index.nu compose up
}
def main [] {
  kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml
}

# def list_files [] {
#     ls
# }

# def 'main create' [] {

# }
