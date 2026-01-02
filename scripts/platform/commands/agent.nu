#!/usr/bin/env nu

# Agent Management Commands
# Manage LLM-powered agents with Dapr integration

use ../../../nu/shared/shared.nu [log]

const AGENTS_DIR = "scripts/platform/agents"
const REGISTRY_FILE = "scripts/platform/agents/registry.json"
const TEMPLATES_DIR = "scripts/platform/agents/templates"

# Active agents tracking (in-memory, use Dapr state for persistence)
mut ACTIVE_AGENTS = []

# List all registered agents
export def "main agent list" [
    --running(-r)  # Show only running agents
    --json(-j)     # Output as JSON
] {
    let registry = open-registry

    let agents = if $running {
        get-running-agents
    } else {
        $registry.agents
    }

    if $json {
        $agents | to json
    } else {
        $agents | select name version description model.default | table
    }
}

# Spawn a new agent instance
export def "main agent spawn" [
    name: string                           # Agent name from registry
    --model(-m): string = ""               # Override default model
    --instance-id(-i): string = ""         # Custom instance ID
    --dapr                                 # Run with Dapr sidecar
] {
    let registry = open-registry
    let agent = $registry.agents | where name == $name | first

    if ($agent | is-empty) {
        log error $"Agent not found: ($name)"
        return
    }

    let instance = if $instance_id == "" {
        $"($name)-(random uuid | str substring 0..8)"
    } else {
        $instance_id
    }

    let model = if $model == "" {
        $agent.model.default
    } else {
        $model
    }

    log info $"Spawning agent: ($instance)"
    log info $"Model: ($model)"

    if $dapr {
        spawn-with-dapr $agent $instance $model
    } else {
        spawn-standalone $agent $instance $model
    }
}

# Stop a running agent
export def "main agent stop" [
    instance_id: string  # Agent instance ID
    --force(-f)          # Force stop without graceful shutdown
] {
    log info $"Stopping agent: ($instance_id)"

    if $force {
        # Kill the process directly
        pkill -f $"agent-($instance_id)"
    } else {
        # Graceful shutdown via Dapr
        try {
            http post $"http://localhost:3500/v1.0/invoke/($instance_id)/method/shutdown" {}
        } catch {
            log error "Graceful shutdown failed, use --force"
        }
    }
}

# View agent logs
export def "main agent logs" [
    instance_id: string  # Agent instance ID
    --follow(-f)         # Follow log output
    --tail(-n): int = 100 # Number of lines
] {
    let log_file = $"/tmp/agent-($instance_id).log"

    if not ($log_file | path exists) {
        log error $"Log file not found: ($log_file)"
        return
    }

    if $follow {
        tail -f $log_file
    } else {
        tail -n $tail $log_file
    }
}

# Invoke an agent with a prompt
export def "main agent invoke" [
    instance_id: string  # Agent instance ID
    prompt: string       # Prompt to send
    --stream(-s)         # Stream response
] {
    log info $"Invoking agent: ($instance_id)"

    let payload = { prompt: $prompt, stream: $stream }

    try {
        let response = http post $"http://localhost:3500/v1.0/invoke/($instance_id)/method/invoke" $payload
        print $response
    } catch {
        # Try direct invocation without Dapr
        try {
            let response = http post $"http://localhost:3001/invoke" $payload
            print $response
        } catch {
            log error "Failed to invoke agent"
        }
    }
}

# Get agent status
export def "main agent status" [
    instance_id: string  # Agent instance ID
] {
    try {
        let status = http get $"http://localhost:3500/v1.0/invoke/($instance_id)/method/status"
        $status | table
    } catch {
        log error $"Agent not responding: ($instance_id)"
    }
}

# Create a new agent definition
export def "main agent create" [
    name: string  # Agent name
    --model(-m): string = "anthropic/claude-3.5-sonnet"  # Default model
] {
    let output_path = $"($AGENTS_DIR)/($name)"

    if ($output_path | path exists) {
        log error $"Agent already exists: ($name)"
        return
    }

    log info $"Creating agent: ($name)"

    mkdir $output_path

    # Create agent.ts
    create-agent-ts $name $output_path $model

    # Create package.json
    {
        name: $name
        version: "0.1.0"
        type: "module"
        main: "agent.ts"
        scripts: {
            start: "tsx agent.ts"
            dev: "tsx watch agent.ts"
        }
        dependencies: {
            "@dapr/dapr": "^3.0.0"
            openai: "^4.0.0"
        }
        devDependencies: {
            typescript: "^5.0.0"
            tsx: "^4.0.0"
            "@types/node": "^20.0.0"
        }
    } | save $"($output_path)/package.json"

    # Create agent.json config
    {
        name: $name
        version: "0.1.0"
        description: $"($name) - LLM-powered agent"
        model: {
            provider: "openrouter"
            default: $model
            fallback: ["openai/gpt-4", "anthropic/claude-3-haiku"]
        }
        tools: []
        systemPrompt: "You are a helpful assistant."
        config: {
            maxTokens: 4096
            temperature: 0.7
            maxTurns: 10
        }
    } | save $"($output_path)/agent.json"

    log info $"Agent created at: ($output_path)"
    log info "Next steps:"
    log info "  1. cd ($output_path) && npm install"
    log info "  2. Edit agent.json to configure"
    log info "  3. nu agent.nu register ($output_path)"
}

# Register an agent in the registry
export def "main agent register" [
    path: string  # Path to agent directory
] {
    let agent_json = $"($path)/agent.json"

    if not ($agent_json | path exists) {
        log error $"agent.json not found at ($path)"
        return
    }

    let agent = open $agent_json
    let registry = open-registry

    # Check for duplicates
    let existing = $registry.agents | where name == $agent.name
    if ($existing | length) > 0 {
        log error $"Agent '($agent.name)' already registered."
        return
    }

    # Add path to agent
    let agent_with_path = $agent | insert path $path

    # Add to registry
    let updated = {
        agents: ($registry.agents | append $agent_with_path)
    }

    $updated | save -f $REGISTRY_FILE

    log info $"Registered agent: ($agent.name)"
}

# Delete an agent
export def "main agent delete" [
    name: string  # Agent name
    --purge       # Also delete agent files
] {
    let registry = open-registry
    let agent = $registry.agents | where name == $name | first

    if ($agent | is-empty) {
        log error $"Agent not found: ($name)"
        return
    }

    # Remove from registry
    let updated = {
        agents: ($registry.agents | where name != $name)
    }
    $updated | save -f $REGISTRY_FILE

    log info $"Removed agent from registry: ($name)"

    if $purge and ($agent.path? | is-not-empty) {
        if ($agent.path | path exists) {
            rm -rf $agent.path
            log info $"Deleted agent files: ($agent.path)"
        }
    }
}

# Helper functions

def open-registry [] {
    if ($REGISTRY_FILE | path exists) {
        open $REGISTRY_FILE
    } else {
        { agents: [] }
    }
}

def get-running-agents [] {
    # Query Dapr for running agent instances
    try {
        let apps = dapr list | lines | skip 1 | parse "{app_id} {http_port} {grpc_port} {app_port} {command}"
        $apps | where app_id =~ "^agent-"
    } catch {
        []
    }
}

def spawn-with-dapr [agent: record, instance: string, model: string] {
    let agent_path = $agent.path? | default $"($AGENTS_DIR)/templates/llm-agent"

    dapr run --app-id $instance --app-port 3001 --dapr-http-port 3500 --components-path $"($AGENTS_DIR)/config/dapr" -- npm start out> $"/tmp/agent-($instance).log" err>| save -a $"/tmp/agent-($instance).log" &

    log info $"Agent spawned with Dapr: ($instance)"
    log info $"Logs: /tmp/agent-($instance).log"
}

def spawn-standalone [agent: record, instance: string, model: string] {
    let agent_path = $agent.path? | default $"($AGENTS_DIR)/templates/llm-agent"

    cd $agent_path
    $env.AGENT_ID = $instance
    $env.AGENT_MODEL = $model
    npm start out> $"/tmp/agent-($instance).log" err>| save -a $"/tmp/agent-($instance).log" &

    log info $"Agent spawned: ($instance)"
}

def create-agent-ts [name: string, path: string, model: string] {
    $"import { DaprClient, DaprServer } from '@dapr/dapr';
import OpenAI from 'openai';

interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
}

class Agent {
  private dapr: DaprClient;
  private server: DaprServer;
  private openai: OpenAI;
  private instanceId: string;
  private model: string;
  private context: Message[] = [];

  constructor\\(\\) {
    this.instanceId = process.env.AGENT_ID || '($name)';
    this.model = process.env.AGENT_MODEL || '($model)';
    this.dapr = new DaprClient\\(\\);
    this.server = new DaprServer\\(\\);
    this.openai = new OpenAI\\({
      baseURL: 'https://openrouter.ai/api/v1',
      apiKey: process.env.OPENROUTER_API_KEY,
    }\\);
  }

  async start\\(\\): Promise<void> {
    // Load persisted state
    await this.loadState\\(\\);

    // Register endpoints
    this.server.invoker.listen\\('invoke', this.handleInvoke.bind\\(this\\), { method: ['POST'] }\\);
    this.server.invoker.listen\\('status', this.handleStatus.bind\\(this\\), { method: ['GET'] }\\);
    this.server.invoker.listen\\('shutdown', this.handleShutdown.bind\\(this\\), { method: ['POST'] }\\);

    await this.server.start\\(\\);
    console.log\\(`Agent \\${this.instanceId} started`\\);
  }

  private async handleInvoke\\(data: { prompt: string }\\): Promise<string> {
    this.context.push\\({ role: 'user', content: data.prompt }\\);

    const response = await this.openai.chat.completions.create\\({
      model: this.model,
      messages: this.context,
    }\\);

    const reply = response.choices[0].message.content || '';
    this.context.push\\({ role: 'assistant', content: reply }\\);

    await this.saveState\\(\\);
    return reply;
  }

  private async handleStatus\\(\\): Promise<object> {
    return {
      instanceId: this.instanceId,
      model: this.model,
      contextLength: this.context.length,
      status: 'running',
    };
  }

  private async handleShutdown\\(\\): Promise<void> {
    await this.saveState\\(\\);
    console.log\\('Agent shutting down...'\\);
    process.exit\\(0\\);
  }

  private async loadState\\(\\): Promise<void> {
    try {
      const state = await this.dapr.state.get\\('statestore', `agent:\\${this.instanceId}`\\);
      if \\(state\\) {
        this.context = state.context || [];
      }
    } catch \\(e\\) {
      console.log\\('No previous state found'\\);
    }
  }

  private async saveState\\(\\): Promise<void> {
    await this.dapr.state.save\\('statestore', [
      { key: `agent:\\${this.instanceId}`, value: { context: this.context } },
    ]\\);
  }
}

const agent = new Agent\\(\\);
agent.start\\(\\).catch\\(console.error\\);
" | save $"($path)/agent.ts"
}

# Main entry point
def main [] {
    print "Agent Management"
    print ""
    print "Commands:"
    print "  agent list        - List all registered agents"
    print "  agent create      - Create a new agent definition"
    print "  agent register    - Register an agent"
    print "  agent spawn       - Start an agent instance"
    print "  agent stop        - Stop a running agent"
    print "  agent status      - Get agent status"
    print "  agent invoke      - Send prompt to agent"
    print "  agent logs        - View agent logs"
    print "  agent delete      - Remove an agent"
}
