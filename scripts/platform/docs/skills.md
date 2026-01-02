# Skills Development Guide

## Overview

Skills are reusable capabilities that can be invoked by agents or directly via CLI. They come in three flavors:

| Type | Runtime | Best For |
|------|---------|----------|
| Rust | Native binary | CPU-intensive work (parsing, processing, compilation) |
| TypeScript | Node.js | LLM integration, API calls, orchestration |
| Nushell | Nu interpreter | Quick scripts, system tasks, prototyping |

## Skill Structure

### Registry Entry

Every skill must be registered in `skills/registry.json`:

```json
{
  "name": "code-review",
  "version": "1.0.0",
  "type": "typescript",
  "description": "AI-powered code review",
  "entry": "./examples/code-review/src/index.ts",
  "capabilities": ["git", "llm"],
  "config": {
    "model": "openai/gpt-4",
    "maxTokens": 4096
  }
}
```

### Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["name", "version", "type", "entry"],
  "properties": {
    "name": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" },
    "version": { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$" },
    "type": { "enum": ["rust", "typescript", "nushell"] },
    "description": { "type": "string" },
    "entry": { "type": "string" },
    "capabilities": { "type": "array", "items": { "type": "string" } },
    "config": { "type": "object" }
  }
}
```

## Creating Skills

### Rust Skill

For CPU-intensive workloads:

```bash
# Create from template
nu skill.nu create my-processor --type rust

# Structure
skills/my-processor/
├── Cargo.toml
├── src/
│   └── main.rs
└── skill.json
```

**src/main.rs:**
```rust
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

#[derive(Deserialize)]
struct Input {
    files: Vec<String>,
    options: ProcessOptions,
}

#[derive(Deserialize)]
struct ProcessOptions {
    parallel: bool,
    chunk_size: usize,
}

#[derive(Serialize)]
struct Output {
    processed: usize,
    results: Vec<ProcessResult>,
}

fn main() -> anyhow::Result<()> {
    // Read JSON input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let input: Input = serde_json::from_str(&input)?;

    // Process files (CPU-intensive)
    let results = if input.options.parallel {
        process_parallel(&input.files, input.options.chunk_size)
    } else {
        process_sequential(&input.files)
    };

    // Output JSON to stdout
    let output = Output {
        processed: results.len(),
        results,
    };
    println!("{}", serde_json::to_string(&output)?);

    Ok(())
}
```

### TypeScript Skill

For LLM-powered and API-intensive work:

```bash
# Create from template
nu skill.nu create code-review --type typescript

# Structure
skills/code-review/
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts
└── skill.json
```

**src/index.ts:**
```typescript
import OpenAI from 'openai';

interface ReviewInput {
  diff: string;
  context?: string;
  model?: string;
}

interface ReviewOutput {
  summary: string;
  issues: Issue[];
  suggestions: string[];
}

const openai = new OpenAI({
  baseURL: 'https://openrouter.ai/api/v1',
  apiKey: process.env.OPENROUTER_API_KEY,
});

async function review(input: ReviewInput): Promise<ReviewOutput> {
  const response = await openai.chat.completions.create({
    model: input.model || 'openai/gpt-4',
    messages: [
      {
        role: 'system',
        content: 'You are a code reviewer. Analyze the diff and provide feedback.',
      },
      {
        role: 'user',
        content: `Review this diff:\n\n${input.diff}\n\nContext: ${input.context || 'None'}`,
      },
    ],
  });

  // Parse structured response
  return parseReviewResponse(response.choices[0].message.content);
}

// Entry point - reads from stdin, writes to stdout
async function main() {
  const input = JSON.parse(await readStdin());
  const output = await review(input);
  console.log(JSON.stringify(output));
}

main().catch(console.error);
```

### Nushell Skill

For quick scripts and system tasks:

```bash
# Create from template
nu skill.nu create file-organizer --type nushell

# Structure
skills/file-organizer/
├── skill.nu
└── skill.json
```

**skill.nu:**
```nushell
#!/usr/bin/env nu

# Organize files by extension
def main [
    --source(-s): path   # Source directory
    --target(-t): path   # Target directory
    --dry-run(-n)        # Preview only
] {
    let files = ls $source | where type == file

    $files | each { |file|
        let ext = $file.name | path parse | get extension
        let dest = $target | path join $ext

        if $dry_run {
            print $"Would move ($file.name) -> ($dest)"
        } else {
            mkdir $dest
            mv $file.name $dest
        }
    }

    { organized: ($files | length), dry_run: $dry_run }
}
```

## Skill Commands

```bash
# List all registered skills
nu skill.nu list

# Create new skill from template
nu skill.nu create <name> --type <rust|typescript|nushell>

# Register existing skill
nu skill.nu register ./path/to/skill

# Run a skill
nu skill.nu run code-review --input '{"diff": "..."}'

# Run with file input
nu skill.nu run file-processor --input-file data.json

# Delete a skill
nu skill.nu delete my-skill
```

## Skill Interface Protocol

All skills follow a standard I/O protocol:

1. **Input**: JSON via stdin or `--input` flag
2. **Output**: JSON to stdout
3. **Errors**: JSON to stderr with exit code 1

```json
// Success output
{
  "status": "success",
  "data": { ... }
}

// Error output (stderr)
{
  "status": "error",
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid input format"
  }
}
```

## Dapr Integration

Skills can optionally integrate with Dapr for:

- State persistence
- Pub/sub messaging
- Secret access

```typescript
import { DaprClient } from '@dapr/dapr';

const dapr = new DaprClient();

// Save skill state
await dapr.state.save('statestore', [
  { key: 'skill-config', value: config }
]);

// Publish completion event
await dapr.pubsub.publish('events', 'skill.completed', {
  skillId: 'code-review',
  result: output
});
```

## Best Practices

1. **Choose the right runtime**:
   - Rust for CPU-bound work (>100ms compute)
   - TypeScript for I/O-bound work (API calls, LLM)
   - Nushell for quick scripts (<50 lines)

2. **Keep skills focused**: One skill, one responsibility

3. **Use structured I/O**: Always JSON, with clear schemas

4. **Handle errors gracefully**: Return error JSON, don't crash

5. **Document capabilities**: List what the skill can do in registry

6. **Version carefully**: Semantic versioning for breaking changes
