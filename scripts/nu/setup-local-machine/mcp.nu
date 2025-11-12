#!/usr/bin/env nu

use std log

# Generate MCP server configuration files
export def --env "main" [
    --location: list<string> = [".mcp.json"]
    --memory-file-path: string = ""
    --anthropic-api-key: string = ""
    --github-token: string = ""
    --enable-playwright  # Switch flag - use without value
] {
    let resolved_memory_file_path = if ($memory_file_path | is-empty) {
        pwd | path join "memory.json" | path expand
    } else {
        $memory_file_path
    }

    let resolved_anthropic_api_key = (
        if ($anthropic_api_key | is-not-empty) { $anthropic_api_key }
        else if ("ANTHROPIC_API_KEY" in $env) { $env.ANTHROPIC_API_KEY }
        else { "" }
    )

    let resolved_github_token = (
        if ($github_token | is-not-empty) { $github_token }
        else if ("GITHUB_TOKEN" in $env) { $env.GITHUB_TOKEN }
        else { "" }
    )

    mut mcp_servers_map = {
        context7: {
            command: "npx",
            args: ["-y", "@upstash/context7-mcp"]
        }
    }

    if ($resolved_anthropic_api_key | is-not-empty) {
        $mcp_servers_map = $mcp_servers_map | upsert "taskmaster-ai" {
            command: "npx",
            args: ["-y", "--package=task-master-ai", "task-master-ai"],
            env: {
                ANTHROPIC_API_KEY: $resolved_anthropic_api_key
            }
        }
    }

    if ($resolved_github_token | is-not-empty) {
        $mcp_servers_map = $mcp_servers_map | upsert "github" {
            url: "https://api.githubcopilot.com/mcp/",
            headers: {
                Authorization: $"Bearer ($resolved_github_token)"
            }
        }
    }

    if $enable_playwright {
        $mcp_servers_map = $mcp_servers_map | upsert "playwright" {
            command: "npx",
            args: ["-y", "@playwright/mcp@latest"]
        }
    }

    let config_record = { mcpServers: $mcp_servers_map }

    for $output_location in $location {
        let parent_dir = $output_location | path dirname

        if not ($parent_dir | path exists) {
            mkdir $parent_dir
            log info $"Created directory: ($parent_dir)"
        }

        $config_record | to json --indent 2 | save -f $output_location
        log info $"MCP servers configuration file created at: ($output_location)"
    }
}
