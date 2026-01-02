# Agents Development Guide

## Overview

Agents are LLM-powered autonomous actors that can:

- Invoke skills based on goals
- Maintain conversation context
- Communicate with other agents
- Persist state across sessions

## Agent Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         Agent                                │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Context   │  │    Tools    │  │    Communication    │  │
│  │  (Memory)   │  │  (Skills)   │  │   (Dapr Pub/Sub)    │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         └────────────────┼─────────────────────┘             │
│                          ▼                                   │
│                   ┌─────────────┐                            │
│                   │  LLM Core   │                            │
│                   │ (OpenRouter)│                            │
│                   └─────────────┘                            │
│                          │                                   │
│                          ▼                                   │
│                   ┌─────────────┐                            │
│                   │Dapr Sidecar │                            │
│                   │  (State,    │                            │
│                   │   Events)   │                            │
│                   └─────────────┘                            │
└─────────────────────────────────────────────────────────────┘
```

## Agent Registry

Register agents in `agents/registry.json`:

```json
{
  "agents": [
    {
      "name": "code-assistant",
      "version": "1.0.0",
      "description": "AI coding assistant with tool access",
      "model": {
        "provider": "openrouter",
        "default": "anthropic/claude-3.5-sonnet",
        "fallback": ["openai/gpt-4", "anthropic/claude-3-haiku"]
      },
      "tools": ["code-review", "file-processor", "git-operations"],
      "systemPrompt": "You are a helpful coding assistant...",
      "config": {
        "maxTokens": 4096,
        "temperature": 0.7,
        "maxTurns": 10
      }
    }
  ]
}
```

## Creating an Agent

### From Template

```bash
# Create new agent
nu agent.nu create my-agent

# Structure
agents/my-agent/
├── agent.ts          # Main agent logic
├── tools.ts          # Tool definitions
├── prompts.ts        # System prompts
├── package.json
└── agent.json        # Agent config
```

### Agent Implementation

**agent.ts:**
```typescript
import { DaprClient } from '@dapr/dapr';
import OpenAI from 'openai';

interface AgentConfig {
  name: string;
  model: string;
  tools: string[];
  systemPrompt: string;
}

interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  toolCalls?: ToolCall[];
}

class Agent {
  private dapr: DaprClient;
  private openai: OpenAI;
  private config: AgentConfig;
  private context: Message[] = [];

  constructor(config: AgentConfig) {
    this.config = config;
    this.dapr = new DaprClient();
    this.openai = new OpenAI({
      baseURL: 'https://openrouter.ai/api/v1',
      apiKey: process.env.OPENROUTER_API_KEY,
    });
  }

  async initialize(): Promise<void> {
    // Load persisted state
    const state = await this.dapr.state.get('statestore', `agent:${this.config.name}`);
    if (state) {
      this.context = state.context || [];
    }

    // Add system prompt
    if (this.context.length === 0) {
      this.context.push({
        role: 'system',
        content: this.config.systemPrompt,
      });
    }
  }

  async invoke(prompt: string): Promise<string> {
    // Add user message
    this.context.push({ role: 'user', content: prompt });

    // Call LLM
    const response = await this.openai.chat.completions.create({
      model: this.config.model,
      messages: this.context,
      tools: await this.getToolDefinitions(),
    });

    const message = response.choices[0].message;

    // Handle tool calls
    if (message.tool_calls) {
      for (const toolCall of message.tool_calls) {
        const result = await this.executeSkill(
          toolCall.function.name,
          JSON.parse(toolCall.function.arguments)
        );

        this.context.push({
          role: 'tool',
          content: JSON.stringify(result),
        });
      }

      // Get final response after tool execution
      return this.invoke('');
    }

    // Add assistant response
    this.context.push({ role: 'assistant', content: message.content });

    // Persist state
    await this.saveState();

    return message.content;
  }

  private async executeSkill(name: string, input: any): Promise<any> {
    // Invoke skill via Dapr service invocation
    return await this.dapr.invoker.invoke(
      name,
      'execute',
      { method: 'POST', body: input }
    );
  }

  private async saveState(): Promise<void> {
    await this.dapr.state.save('statestore', [
      {
        key: `agent:${this.config.name}`,
        value: { context: this.context },
      },
    ]);
  }

  async stop(): Promise<void> {
    await this.saveState();
    // Publish shutdown event
    await this.dapr.pubsub.publish('events', 'agent.stopped', {
      agentId: this.config.name,
      timestamp: new Date().toISOString(),
    });
  }
}

export { Agent };
```

## Agent Commands

```bash
# List all agents
nu agent.nu list

# Spawn a new agent instance
nu agent.nu spawn code-assistant --model anthropic/claude-3.5-sonnet

# Check agent status
nu agent.nu status code-assistant

# Invoke agent with a prompt
nu agent.nu invoke code-assistant "Review the latest commit"

# View agent logs
nu agent.nu logs code-assistant

# Stop agent
nu agent.nu stop code-assistant

# Delete agent definition
nu agent.nu delete code-assistant
```

## Agent-to-Agent Communication

Agents communicate via Dapr pub/sub:

```typescript
// Subscribe to messages from other agents
@DaprSubscribe({ pubsubName: 'events', topic: 'agent.message' })
async handleMessage(event: AgentMessage) {
  if (event.target === this.config.name) {
    const response = await this.invoke(event.content);

    // Reply to sender
    await this.dapr.pubsub.publish('events', 'agent.message', {
      from: this.config.name,
      target: event.from,
      content: response,
    });
  }
}

// Send message to another agent
async sendMessage(targetAgent: string, content: string): Promise<void> {
  await this.dapr.pubsub.publish('events', 'agent.message', {
    from: this.config.name,
    target: targetAgent,
    content,
  });
}
```

## Tool Binding

Define which skills an agent can use:

**tools.ts:**
```typescript
import { Tool } from 'openai/resources/chat/completions';

export const tools: Tool[] = [
  {
    type: 'function',
    function: {
      name: 'code-review',
      description: 'Review code changes and provide feedback',
      parameters: {
        type: 'object',
        properties: {
          diff: { type: 'string', description: 'Git diff to review' },
          context: { type: 'string', description: 'Additional context' },
        },
        required: ['diff'],
      },
    },
  },
  {
    type: 'function',
    function: {
      name: 'file-processor',
      description: 'Process files with CPU-intensive operations',
      parameters: {
        type: 'object',
        properties: {
          files: { type: 'array', items: { type: 'string' } },
          operation: { type: 'string', enum: ['parse', 'transform', 'analyze'] },
        },
        required: ['files', 'operation'],
      },
    },
  },
];
```

## Best Practices

1. **Keep context manageable**: Trim old messages to stay within token limits

2. **Use tool fallbacks**: If a skill fails, have a backup strategy

3. **Persist important state**: Use Dapr state store for durability

4. **Handle rate limits**: Implement exponential backoff for LLM calls

5. **Log interactions**: Track agent decisions for debugging

6. **Set boundaries**: Limit max turns, token usage, and skill invocations

7. **Test thoroughly**: Mock LLM responses for deterministic testing
