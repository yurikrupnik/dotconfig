#!/usr/bin/env nu
# Disposable Lima VMs that run ./install.sh against this repo's working tree
# (tracked + untracked, non-ignored files; .env, output/, tmp/ stay on the host,
# and no host directory is mounted into the guest).
#
#   nu scripts/nu/sandbox.nu linux             # Ubuntu 24.04: install.sh, then a shell; VM deleted on exit
#   nu scripts/nu/sandbox.nu mac               # macOS guest (first run restores an IPSW: ~20 GB, ~20 min)
#   nu scripts/nu/sandbox.nu linux --keep      # keep the VM; the next run reuses it (re-run = idempotency test)
#   nu scripts/nu/sandbox.nu linux --fresh     # discard a kept VM, start from the pristine base again
#   nu scripts/nu/sandbox.nu linux --shell     # skip install.sh, just open a shell
#   nu scripts/nu/sandbox.nu linux --template debian-13
#   nu scripts/nu/sandbox.nu clean             # delete every sandbox VM, bases included
#
# Each OS + template gets a pristine base VM (`dotconfig-<os>-base-<template>`, created once, never
# touched by install.sh); every run clones it, so only the first run pays for
# the image download and first boot. install.sh prompts before trusting new
# Brewfile taps; set BREW_TRUST_NEW_TAPS=1 on the host for unattended runs.
# Windows: Lima has no Windows guests — see the dotconfig skill's
# references/platforms.md (UTM + Windows 11 ARM).

use worktree.nu

const REPO = path self | path dirname | path dirname | path dirname
const PREFIX = "dotconfig"

const PROFILES = {
    linux: { template: "ubuntu-24.04", cpus: 4, memory: 8, timeout: "15m" }
    mac: { template: "macos", cpus: 4, memory: 8, timeout: "60m" }
}

# Runs inside the guest: unpack the working tree to ~/dotconfig and install.
# macOS guests have a password-protected sudo; Lima writes the password to
# ~/password, and the Homebrew installer honors SUDO_ASKPASS (`sudo -A`).
const GUEST_INSTALL = r##'
set -euo pipefail
rm -rf "$HOME/dotconfig"
mkdir -p "$HOME/dotconfig"
tar -xzf /tmp/dotconfig.tgz -C "$HOME/dotconfig"
rm -f /tmp/dotconfig.tgz
if [ -f "$HOME/password" ]; then
    printf '#!/bin/sh\ncat "$HOME/password"\n' > "$HOME/.sandbox-askpass"
    chmod 700 "$HOME/.sandbox-askpass"
    export SUDO_ASKPASS="$HOME/.sandbox-askpass"
fi
export NONINTERACTIVE=1
cd "$HOME/dotconfig"
./install.sh
'##

def instances []: nothing -> list<string> {
    ^limactl list --quiet | lines
}

def status_of [name: string]: nothing -> string {
    ^limactl list --format '{{.Status}}' $name | str trim
}

def --wrapped lima [...args: string] {
    ^limactl --tty=false ...$args
}

# The base is built under a `-building` name and renamed only once it is
# provisioned and stopped: Lima lists an instance as soon as `create` starts,
# so a concurrent run must never see (and clone) a half-restored base.
def ensure_base [os: string, profile: record, template: string]: nothing -> string {
    let base = $"($PREFIX)-($os)-base-($template)"
    let building = $"($base)-building"
    if $base in (instances) {
        return $base
    }
    if $building in (instances) {
        error make { msg: $"($building) exists: another run is building the base, or a previous build aborted \(`limactl delete --force ($building)`\)" }
    }
    print $"==> Creating pristine base VM ($base) from template:($template)"
    (lima create --name $building --mount-none --containerd none
        --cpus ($profile.cpus | into string) --memory ($profile.memory | into string)
        $"template:($template)")
    # First boot provisions the guest (cloud-init / macOS restore); stop it
    # so it can be cloned.
    lima start --timeout $profile.timeout $building
    lima stop $building
    lima rename $building $base
    $base
}

def sandbox [os: string, template: string, keep: bool, fresh: bool, shell: bool] {
    if (which limactl | is-empty) {
        error make { msg: "limactl not on PATH — `lima` is declared in config/brew/Brewfile: just brew-install" }
    }
    let profile = $PROFILES | get $os
    let template = if ($template | is-empty) { $profile.template } else { $template }
    let name = $"($PREFIX)-($os)"

    if $fresh and $name in (instances) {
        lima delete --force $name
    }
    if $name in (instances) {
        # Sessions stop kept VMs on exit, so a running one belongs to another session.
        if (status_of $name) == "Running" {
            error make { msg: $"($name) is running — another sandbox session uses it \(or stop it: `limactl stop ($name)`\)" }
        }
        print $"==> Reusing kept VM ($name)"
        lima start --timeout $profile.timeout $name
    } else {
        let base = ensure_base $os $profile $template
        print $"==> Cloning ($base) → ($name)"
        lima clone $base $name
        lima start --timeout $profile.timeout $name
    }

    let installed = if $shell { true } else {
        let tgz = worktree pack $REPO
        lima copy $tgz $"($name):/tmp/dotconfig.tgz"
        rm $tgz
        let trust = if ($env.BREW_TRUST_NEW_TAPS? == "1") { "export BREW_TRUST_NEW_TAPS=1\n" } else { "" }
        print $"==> ./install.sh in ($name)"
        try {
            ^limactl shell --workdir / $name bash -c $"($trust)($GUEST_INSTALL)"
            true
        } catch {
            print $"==> install.sh failed in ($name)"
            false
        }
    }

    if (is-terminal --stdin) {
        print $"==> Shell in ($name) — exit to (if $keep { 'keep' } else { 'delete' }) the VM"
        try { ^limactl shell --workdir / $name bash -lc 'cd "$HOME/dotconfig" 2>/dev/null || cd; exec "${SHELL:-bash}" -l' }
    }

    if $keep {
        lima stop $name
        print $"==> Kept ($name) \(stopped\): re-run with --keep to reuse it, or `limactl start ($name) && limactl shell ($name)`"
    } else {
        lima delete --force $name
    }
    if not $installed {
        exit 1
    }
}

# Disposable Lima VMs for testing install.sh on Linux and macOS.
def main [] {
    help main
}

# Ubuntu 24.04 VM (any Lima template via --template).
def "main linux" [
    --template: string = ""  # Lima template name, e.g. debian-13, fedora
    --keep                   # keep the VM after exit; the next run reuses it
    --fresh                  # discard a kept VM first
    --shell                  # skip install.sh, only open a shell
] {
    sandbox linux $template $keep $fresh $shell
}

# macOS VM on Apple Silicon (Virtualization.framework).
def "main mac" [
    --template: string = ""  # Lima template name, e.g. macos-15
    --keep                   # keep the VM after exit; the next run reuses it
    --fresh                  # discard a kept VM first
    --shell                  # skip install.sh, only open a shell
] {
    if $nu.os-info.name != "macos" or $nu.os-info.arch != "aarch64" {
        error make { msg: "macOS guests need an Apple Silicon macOS host" }
    }
    sandbox mac $template $keep $fresh $shell
}

# Delete every sandbox VM, pristine bases included.
def "main clean" [] {
    let ours = instances | where {|n| $n starts-with $"($PREFIX)-" }
    if ($ours | is-empty) {
        print "no sandbox VMs"
        return
    }
    lima delete --force ...$ours
}
