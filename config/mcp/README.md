# `config/mcp/`

Source of truth for MCP server definitions. Edit `servers.json`, then run `mcp` to emit `.mcp.json` (or any location) for an AI agent to load.

## Add a new server

Edit `servers.json`. Adding the server requires **no code change** to `mcp.nu`. The schema mirrors the standard `mcpServers` map used by Claude Code, Codex, Cursor, etc., plus two optional metadata fields prefixed with `_`:

| Field | Purpose |
|---|---|
| `_requires` | List of env var names. If any is empty/unset, the server is skipped. |
| `_enabled`  | `false` ⇒ opt-in via `mcp --enable <name>`. Default is `true`. |

The `_*` fields are stripped from the generated output.

## `${VAR}` substitution

Any string field can reference an env var as `${VAR}`. The value is substituted at generate time. Use this together with `_requires` so servers that need a token aren't emitted half-configured.

```json
"github": {
  "url": "https://api.githubcopilot.com/mcp/",
  "headers": { "Authorization": "Bearer ${GITHUB_TOKEN}" },
  "_requires": ["GITHUB_TOKEN"]
}
```

## Common patterns

**Command-based server (most MCPs):**
```json
"context7": {
  "command": "npx",
  "args": ["-y", "@upstash/context7-mcp"]
}
```

**URL/header-based server (hosted MCP):**
```json
"github": {
  "url": "https://api.githubcopilot.com/mcp/",
  "headers": { "Authorization": "Bearer ${GITHUB_TOKEN}" }
}
```

**Opt-in heavy dependency:**
```json
"playwright": {
  "command": "npx",
  "args": ["-y", "@playwright/mcp@latest"],
  "_enabled": false
}
```
Use with `mcp --enable playwright`.

## Targets

`--target <name>` writes to the right file in the right shape for each AI client. Multiple targets supported (comma-separated).

| Target | Path | Format | Strategy | Key |
|---|---|---|---|---|
| `claude-code` | `./.mcp.json` | json | replace | `mcpServers` |
| `claude-desktop` | `~/Library/Application Support/Claude/claude_desktop_config.json` | json | merge | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | json | replace | `mcpServers` |
| `codex` | `~/.codex/config.toml` | toml | merge | `mcp_servers` |
| `zed` | `~/.config/zed/settings.json` | json | merge | `context_servers` |
| `gemini` | `~/.gemini/settings.json` | json | merge | `mcpServers` |

**`replace`** wipes the file and writes only `{ <key>: <servers> }`. Safe for MCP-only files.
**`merge`** reads the existing file, replaces only the configured key, and writes back — so unrelated settings (Zed theme, Codex model, Claude Desktop's sibling keys) survive.

Zed's `context_servers` shape may need adjustment depending on Zed version — newer versions accept the standard MCP fields directly. If yours doesn't, fork the target's transform in `mcp.nu`.

## CLI

| Command | What it does |
|---|---|
| `mcp` | Write `./.mcp.json` (default target: `claude-code`) |
| `mcp --target cursor,codex` | Write to multiple named targets |
| `mcp --target all-known --list-targets` | Show target registry and which paths exist on this machine |
| `mcp --location a.json,b.json` | Ad-hoc path(s) — JSON, replace strategy, `mcpServers` key |
| `mcp --enable playwright` | Include opt-in servers (comma-separated for several) |
| `mcp --disable browsermcp` | Exclude otherwise-included servers |
| `mcp --list` | Print the server inclusion table without writing |
| `mcp --config path/to/other.json` | Use a different source than `~/dotconfig/config/mcp/servers.json` |
