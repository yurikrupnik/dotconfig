#!/usr/bin/env nu

# Skills Management Commands
# Manage Rust, TypeScript, and Nushell skills

use ../../../nu/shared/shared.nu [log]

const SKILLS_DIR = "scripts/platform/skills"
const REGISTRY_FILE = "scripts/platform/skills/registry.json"
const TEMPLATES_DIR = "scripts/platform/skills/templates"

# List all registered skills
export def "main skill list" [
    --type(-t): string = ""  # Filter by type (rust, typescript, nushell)
    --json(-j)               # Output as JSON
] {
    let registry = open-registry

    let skills = if $type == "" {
        $registry.skills
    } else {
        $registry.skills | where type == $type
    }

    if $json {
        $skills | to json
    } else {
        $skills | select name version type description | table
    }
}

# Create a new skill from template
export def "main skill create" [
    name: string                      # Skill name (lowercase, hyphenated)
    --type(-t): string = "typescript" # Skill type: rust, typescript, nushell
    --path(-p): string = ""           # Custom output path
] {
    let output_path = if $path == "" {
        $"($SKILLS_DIR)/examples/($name)"
    } else {
        $path
    }

    # Validate name
    if not ($name =~ '^[a-z][a-z0-9-]*$') {
        log error $"Invalid skill name: ($name). Use lowercase letters, numbers, and hyphens."
        return
    }

    # Check if exists
    if ($output_path | path exists) {
        log error $"Skill already exists at ($output_path)"
        return
    }

    log info $"Creating ($type) skill: ($name)"

    match $type {
        "rust" => { create-rust-skill $name $output_path }
        "typescript" => { create-ts-skill $name $output_path }
        "nushell" => { create-nu-skill $name $output_path }
        _ => {
            log error $"Unknown skill type: ($type). Use rust, typescript, or nushell."
            return
        }
    }

    log info $"Skill created at: ($output_path)"
    log info $"Register with: nu skill.nu register ($output_path)"
}

# Register a skill in the registry
export def "main skill register" [
    path: string  # Path to skill directory
] {
    let skill_json = $"($path)/skill.json"

    if not ($skill_json | path exists) {
        log error $"skill.json not found at ($path)"
        return
    }

    let skill = open $skill_json
    let registry = open-registry

    # Check for duplicates
    let existing = $registry.skills | where name == $skill.name
    if ($existing | length) > 0 {
        log error $"Skill '($skill.name)' already registered. Use 'skill delete' first."
        return
    }

    # Add to registry
    let updated = {
        skills: ($registry.skills | append $skill)
    }

    $updated | save -f $REGISTRY_FILE

    log info $"Registered skill: ($skill.name) v($skill.version)"
}

# Run a skill
export def "main skill run" [
    name: string       # Skill name
    --input(-i): string = ""      # JSON input string
    --input-file(-f): string = "" # Input file path
] {
    let registry = open-registry
    let skill = $registry.skills | where name == $name | first

    if ($skill | is-empty) {
        log error $"Skill not found: ($name)"
        return
    }

    let input_data = if $input != "" {
        $input
    } else if $input_file != "" {
        open $input_file | to json
    } else {
        "{}"
    }

    log info $"Running skill: ($name) [($skill.type)]"

    match $skill.type {
        "rust" => { run-rust-skill $skill $input_data }
        "typescript" => { run-ts-skill $skill $input_data }
        "nushell" => { run-nu-skill $skill $input_data }
        _ => { log error $"Unknown skill type: ($skill.type)" }
    }
}

# Delete a skill from registry
export def "main skill delete" [
    name: string  # Skill name to delete
    --purge       # Also delete skill files
] {
    let registry = open-registry
    let skill = $registry.skills | where name == $name | first

    if ($skill | is-empty) {
        log error $"Skill not found: ($name)"
        return
    }

    # Remove from registry
    let updated = {
        skills: ($registry.skills | where name != $name)
    }
    $updated | save -f $REGISTRY_FILE

    log info $"Removed skill from registry: ($name)"

    if $purge {
        let skill_path = $skill.entry | path dirname
        if ($skill_path | path exists) {
            rm -rf $skill_path
            log info $"Deleted skill files: ($skill_path)"
        }
    }
}

# Build a Rust skill
export def "main skill build" [
    name: string  # Skill name to build
    --release     # Build in release mode
] {
    let registry = open-registry
    let skill = $registry.skills | where name == $name | first

    if ($skill | is-empty) or ($skill.type != "rust") {
        log error $"Rust skill not found: ($name)"
        return
    }

    let skill_path = $skill.entry | path dirname
    let build_args = if $release { ["--release"] } else { [] }

    log info $"Building ($name)..."
    cd $skill_path
    cargo build ...$build_args
}

# Helper functions

def open-registry [] {
    if ($REGISTRY_FILE | path exists) {
        open $REGISTRY_FILE
    } else {
        { skills: [] }
    }
}

def create-rust-skill [name: string, path: string] {
    mkdir $path
    mkdir $"($path)/src"

    # Cargo.toml
    $'[package]
name = "($name)"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
' | save $"($path)/Cargo.toml"

    # src/main.rs
    $'use serde::\{Deserialize, Serialize\};
use std::io::\{self, Read\};

#[derive\(Deserialize\)]
struct Input \{
    // Define your input fields
    data: String,
\}

#[derive\(Serialize\)]
struct Output \{
    // Define your output fields
    result: String,
    success: bool,
\}

fn main\(\) -> anyhow::Result<\(\)> \{
    // Read JSON input from stdin
    let mut input_str = String::new\(\);
    io::stdin\(\).read_to_string\(&mut input_str\)?;
    let input: Input = serde_json::from_str\(&input_str\)?;

    // Process input \(your logic here\)
    let output = Output \{
        result: format!\("Processed: \{\}", input.data\),
        success: true,
    \};

    // Output JSON to stdout
    println!\("\{\}", serde_json::to_string\(&output\)?\);
    Ok\(\(\)\)
\}
' | save $"($path)/src/main.rs"

    # skill.json
    {
        name: $name
        version: "0.1.0"
        type: "rust"
        description: $"($name) - Rust CPU-intensive skill"
        entry: $"($path)/target/release/($name)"
        capabilities: []
        config: {}
    } | save $"($path)/skill.json"
}

def create-ts-skill [name: string, path: string] {
    mkdir $path
    mkdir $"($path)/src"

    # package.json
    {
        name: $name
        version: "0.1.0"
        type: "module"
        main: "src/index.ts"
        scripts: {
            build: "tsc"
            start: "tsx src/index.ts"
        }
        dependencies: {
            openai: "^4.0.0"
        }
        devDependencies: {
            typescript: "^5.0.0"
            tsx: "^4.0.0"
            "@types/node": "^20.0.0"
        }
    } | save $"($path)/package.json"

    # tsconfig.json
    {
        compilerOptions: {
            target: "ES2022"
            module: "ESNext"
            moduleResolution: "node"
            strict: true
            esModuleInterop: true
            outDir: "dist"
        }
        include: ["src/**/*"]
    } | save $"($path)/tsconfig.json"

    # src/index.ts
    $"import OpenAI from 'openai';

interface Input {
  // Define your input fields
  prompt: string;
}

interface Output {
  // Define your output fields
  result: string;
  success: boolean;
}

const openai = new OpenAI\({
  baseURL: 'https://openrouter.ai/api/v1',
  apiKey: process.env.OPENROUTER_API_KEY,
}\);

async function execute\(input: Input\): Promise<Output> {
  // Your skill logic here
  const response = await openai.chat.completions.create\({
    model: 'anthropic/claude-3-haiku',
    messages: [{ role: 'user', content: input.prompt }],
  }\);

  return {
    result: response.choices[0].message.content || '',
    success: true,
  };
}

// Entry point
async function main\(\) {
  const input = JSON.parse\(await readStdin\(\)\);
  const output = await execute\(input\);
  console.log\(JSON.stringify\(output\)\);
}

function readStdin\(\): Promise<string> {
  return new Promise\(\(resolve\) => {
    let data = '';
    process.stdin.on\('data', \(chunk\) => \(data += chunk\)\);
    process.stdin.on\('end', \(\) => resolve\(data\)\);
  }\);
}

main\(\).catch\(console.error\);
" | save $"($path)/src/index.ts"

    # skill.json
    {
        name: $name
        version: "0.1.0"
        type: "typescript"
        description: $"($name) - TypeScript LLM-powered skill"
        entry: $"($path)/src/index.ts"
        capabilities: ["llm"]
        config: {
            model: "anthropic/claude-3-haiku"
        }
    } | save $"($path)/skill.json"
}

def create-nu-skill [name: string, path: string] {
    mkdir $path

    # skill.nu
    $"#!/usr/bin/env nu

# ($name) - Nushell skill

def main [
    --input\(-i\): string = '{}'  # JSON input
] {
    let data = \\($input | from json\\)

    # Your skill logic here
    let result = {
        processed: true
        input_received: \\$data
        message: 'Skill executed successfully'
    }

    \\$result | to json
}
" | save $"($path)/skill.nu"

    # skill.json
    {
        name: $name
        version: "0.1.0"
        type: "nushell"
        description: $"($name) - Nushell script skill"
        entry: $"($path)/skill.nu"
        capabilities: []
        config: {}
    } | save $"($path)/skill.json"
}

def run-rust-skill [skill: record, input: string] {
    let binary = $skill.entry
    if not ($binary | path exists) {
        log error $"Binary not found. Run: nu skill.nu build ($skill.name) --release"
        return
    }
    echo $input | run-external $binary
}

def run-ts-skill [skill: record, input: string] {
    let entry = $skill.entry
    echo $input | tsx $entry
}

def run-nu-skill [skill: record, input: string] {
    let entry = $skill.entry
    nu $entry --input $input
}

# Main entry point
def main [] {
    print "Skills Management"
    print ""
    print "Commands:"
    print "  skill list        - List all registered skills"
    print "  skill create      - Create a new skill from template"
    print "  skill register    - Register a skill in the registry"
    print "  skill run         - Run a skill"
    print "  skill delete      - Remove a skill"
    print "  skill build       - Build a Rust skill"
}
