#!/usr/bin/env nu

# devkit fleet — self-hosted machine fleet: one hub, push agents, agentless probes.
#
# The app (Bun hub + agent + UI) is bundled in ./fleet/ next to this module.
# The hub runs on one always-on machine on the network; every machine that can
# run Bun reports with `devkit fleet agent`; phones/tablets/IoT that cannot are
# probed by the hub itself ([[fleet.probes]] in devkit.toml). The UI is
# responsive — built for phone/tablet screens as much as desktops.
#
# Trust model: LAN tool. Set fleet.token to gate reports; reads are open.

use common.nu *
use config.nu *

# Directory holding the bundled fleet app (resolved at parse time).
const FLEET_DIR = (path self . | path join "fleet")

# Self-hosted machine fleet. Run a subcommand, or `help devkit fleet <cmd>`.
export def "devkit fleet" [] {
    print "devkit fleet — self-hosted machine fleet: hub, agents, probes"
    print ""
    print "  devkit fleet hub [--port N]          run the hub (on the always-on machine)"
    print "  devkit fleet agent [--hub URL]       report this machine to the hub (blocks)"
    print "  devkit fleet agent --once            single report (smoke test / cron)"
    print "  devkit fleet status                  fleet table from the hub API"
    print "  devkit fleet open                    open the dashboard in the browser"
    print ""
    print "Config ([fleet] in devkit.toml): port, hub, token, interval, db,"
    print "retention_hours, probe_interval, and [[fleet.probes]] rows"
    print "{ name, host, port? } for agentless devices (phones, tablets, IoT)."
}

# Environment the hub process reads its configuration from.
def hub-env [cfg: record, port: int]: nothing -> record {
    {
        FLEET_PORT: ($port | into string)
        FLEET_TOKEN: $cfg.fleet.token
        FLEET_DB: ($cfg.fleet.db | path expand)
        FLEET_RETENTION_HOURS: ($cfg.fleet.retention_hours | into string)
        FLEET_PROBE_INTERVAL: ($cfg.fleet.probe_interval | into string)
        FLEET_STALE_AFTER: ($cfg.fleet.stale_after | into string)
        FLEET_OFFLINE_AFTER: ($cfg.fleet.offline_after | into string)
        FLEET_PROBES: ($cfg.fleet.probes | to json --raw)
    }
}

# Run the fleet hub in the foreground (Ctrl-C to stop).
export def "devkit fleet hub" [
    --port (-p): int = -1   # Listen port (default: config fleet.port)
] {
    require-bin "bun"
    let cfg = (resolve-config)
    let port = (if $port < 0 { $cfg.fleet.port } else { $port })
    info $"Fleet hub on http://localhost:($port)  \(Ctrl-C to stop\)"
    with-env (hub-env $cfg $port) {
        bun run ($FLEET_DIR | path join "hub.ts")
    }
}

# Report this machine to the hub. Blocks; use --once for a single report.
export def "devkit fleet agent" [
    --hub (-u): string      # Hub URL (default: config fleet.hub)
    --interval (-i): int = -1  # Report interval seconds (default: config fleet.interval)
    --once                  # Send one report and exit
] {
    require-bin "bun"
    let cfg = (resolve-config)
    let hub = (if ($hub | is-empty) { $cfg.fleet.hub } else { $hub })
    let interval = (if $interval < 0 { $cfg.fleet.interval } else { $interval })
    let env_rec = {
        FLEET_HUB: $hub
        FLEET_TOKEN: $cfg.fleet.token
        FLEET_INTERVAL: ($interval | into string)
    }
    with-env $env_rec {
        if $once {
            bun run ($FLEET_DIR | path join "agent.ts") -- --once
        } else {
            bun run ($FLEET_DIR | path join "agent.ts")
        }
    }
}

# Fleet overview as a table (from the hub API).
export def "devkit fleet status" [
    --hub (-u): string      # Hub URL (default: config fleet.hub)
] {
    let cfg = (resolve-config)
    let hub = (if ($hub | is-empty) { $cfg.fleet.hub } else { $hub })
    let fleet = (try { http get $"($hub)/api/fleet" } catch {
        error $"Hub unreachable at ($hub). Start it with: devkit fleet hub"
        exit 1
    })
    $fleet.machines | each {|m|
        {
            machine: $m.hostname
            kind: $m.kind
            status: $m.status
            "cpu%": ($m.latest?.cpu? | default null | if ($in == null) { "" } else { $in | math round --precision 1 })
            "latency ms": ($m.latest?.latency_ms? | default null | if ($in == null) { "" } else { $in | math round --precision 1 })
            platform: ($m.platform | default "")
            last_seen: ($m.last_seen * 1_000_000_000 | into datetime)
        }
    }
}

# Open the fleet dashboard in the browser.
export def "devkit fleet open" [
    --hub (-u): string      # Hub URL (default: config fleet.hub)
] {
    let cfg = (resolve-config)
    let hub = (if ($hub | is-empty) { $cfg.fleet.hub } else { $hub })
    info $"Dashboard: ($hub)"
    if (is-macos) { ^open $hub } else { ^xdg-open $hub }
}
