#!/usr/bin/env nu

#use mcp.nu
#source shells.nu

def main [] {
    generate-local-machine-setup
}

def 'generate-local-machine-setup' [] {
    brew bundle --file ~/dotconfig/brew/Brewfile
    rustup update
    gcloud components update
    nu ~/dotconfig/scripts/nu/setup-local-machine/shells.nu generate
#    npm update -g
}
