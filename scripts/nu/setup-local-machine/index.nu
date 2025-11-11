#!/usr/bin/env nu

use mcp.nu

def 'main update' [] {
    brew bundle --file ~/dotconfig/brew/Brewfile
    rustup update
    gcloud components update
    npm update -g
}
