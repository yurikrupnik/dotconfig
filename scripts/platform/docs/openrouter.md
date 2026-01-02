# OpenRouter Integration Guide

## Overview

OpenRouter provides unified access to multiple LLM providers through a single API:

- **Model routing**: Access OpenAI, Anthropic, Google, Meta, and more
- **Automatic fallback**: Failover to backup models
- **Cost optimization**: Route to cheaper models for simple tasks
- **Rate limit handling**: Automatic retry with backoff

## Configuration

**agents/config/openrouter.yaml:**
```yaml
openrouter:
  baseUrl: https://openrouter.ai/api/v1

  # Default model settings
  defaults:
    model: anthropic/claude-3.5-sonnet
    maxTokens: 4096
    temperature: 0.7

  # Model routing rules
  routing:
    # High-quality tasks
    complex:
      primary: anthropic/claude-3.5-sonnet
      fallback:
        - openai/gpt-4-turbo
        - anthropic/claude-3-opus
      maxTokens: 8192

    # Fast/cheap tasks
    simple:
      primary: anthropic/claude-3-haiku
      fallback:
        - openai/gpt-3.5-turbo
        - mistralai/mistral-7b-instruct
      maxTokens: 2048

    # Code-specific tasks
    coding:
      primary: anthropic/claude-3.5-sonnet
      fallback:
        - openai/gpt-4-turbo
        - deepseek/deepseek-coder
      maxTokens: 8192

  # Rate limiting
  rateLimits:
    requestsPerMinute: 60
    tokensPerMinute: 100000

  # Retry configuration
  retry:
    maxAttempts: 3
    initialDelayMs: 1000
    maxDelayMs: 10000
    backoffMultiplier: 2
```

## Usage

### Basic Client

```typescript
import OpenAI from 'openai';

const openai = new OpenAI({
  baseURL: 'https://openrouter.ai/api/v1',
  apiKey: process.env.OPENROUTER_API_KEY,
  defaultHeaders: {
    'HTTP-Referer': 'https://your-app.com',
    'X-Title': 'Your App Name',
  },
});

// Simple completion
const response = await openai.chat.completions.create({
  model: 'anthropic/claude-3.5-sonnet',
  messages: [
    { role: 'user', content: 'Hello!' },
  ],
});
```

### With Fallback

```typescript
import { OpenRouterClient } from './openrouter-client';

const client = new OpenRouterClient({
  apiKey: process.env.OPENROUTER_API_KEY,
  routing: 'complex',  // Uses complex routing rules
});

// Automatically falls back if primary model fails
const response = await client.chat({
  messages: [{ role: 'user', content: 'Analyze this code...' }],
});
```

### Custom Router

```typescript
interface RouterConfig {
  primary: string;
  fallback: string[];
  maxRetries: number;
}

class ModelRouter {
  private openai: OpenAI;
  private config: RouterConfig;

  constructor(config: RouterConfig) {
    this.config = config;
    this.openai = new OpenAI({
      baseURL: 'https://openrouter.ai/api/v1',
      apiKey: process.env.OPENROUTER_API_KEY,
    });
  }

  async complete(messages: Message[]): Promise<string> {
    const models = [this.config.primary, ...this.config.fallback];

    for (let i = 0; i < models.length; i++) {
      try {
        const response = await this.openai.chat.completions.create({
          model: models[i],
          messages,
        });
        return response.choices[0].message.content;
      } catch (error) {
        if (i === models.length - 1) throw error;
        console.warn(`Model ${models[i]} failed, trying fallback...`);
      }
    }

    throw new Error('All models failed');
  }
}
```

## Model Selection Guide

| Use Case | Recommended Model | Fallback |
|----------|-------------------|----------|
| Complex reasoning | `anthropic/claude-3.5-sonnet` | `openai/gpt-4-turbo` |
| Code generation | `anthropic/claude-3.5-sonnet` | `deepseek/deepseek-coder` |
| Quick responses | `anthropic/claude-3-haiku` | `openai/gpt-3.5-turbo` |
| Long context | `anthropic/claude-3.5-sonnet` | `google/gemini-pro-1.5` |
| Cost-sensitive | `mistralai/mistral-7b-instruct` | `openai/gpt-3.5-turbo` |
| Creative writing | `anthropic/claude-3-opus` | `openai/gpt-4` |

## Available Models

Query available models:

```bash
curl https://openrouter.ai/api/v1/models \
  -H "Authorization: Bearer $OPENROUTER_API_KEY"
```

Popular models:
- `anthropic/claude-3.5-sonnet` - Best overall
- `anthropic/claude-3-opus` - Most capable
- `anthropic/claude-3-haiku` - Fastest
- `openai/gpt-4-turbo` - Strong reasoning
- `openai/gpt-3.5-turbo` - Fast and cheap
- `google/gemini-pro-1.5` - Long context
- `meta-llama/llama-3.1-405b-instruct` - Open source large
- `deepseek/deepseek-coder` - Code specialist

## Cost Management

```typescript
interface UsageTracker {
  trackUsage(model: string, tokens: { prompt: number; completion: number }): void;
  getUsage(): UsageReport;
}

class CostAwareRouter {
  private budget: number;
  private spent: number = 0;
  private modelCosts: Record<string, number>;

  constructor(dailyBudget: number) {
    this.budget = dailyBudget;
    this.modelCosts = {
      'anthropic/claude-3-opus': 15.0,
      'anthropic/claude-3.5-sonnet': 3.0,
      'anthropic/claude-3-haiku': 0.25,
      'openai/gpt-4-turbo': 10.0,
      'openai/gpt-3.5-turbo': 0.5,
    };
  }

  selectModel(complexity: 'low' | 'medium' | 'high'): string {
    const remainingBudget = this.budget - this.spent;

    if (complexity === 'low' || remainingBudget < 1) {
      return 'anthropic/claude-3-haiku';
    }
    if (complexity === 'medium' || remainingBudget < 5) {
      return 'anthropic/claude-3.5-sonnet';
    }
    return 'anthropic/claude-3-opus';
  }

  recordUsage(model: string, tokens: number): void {
    const costPer1M = this.modelCosts[model] || 1.0;
    this.spent += (tokens / 1_000_000) * costPer1M;
  }
}
```

## Streaming

```typescript
const stream = await openai.chat.completions.create({
  model: 'anthropic/claude-3.5-sonnet',
  messages: [{ role: 'user', content: 'Tell me a story...' }],
  stream: true,
});

for await (const chunk of stream) {
  const content = chunk.choices[0]?.delta?.content;
  if (content) {
    process.stdout.write(content);
  }
}
```

## Tool Use

```typescript
const response = await openai.chat.completions.create({
  model: 'anthropic/claude-3.5-sonnet',
  messages: [{ role: 'user', content: 'What files are in the current directory?' }],
  tools: [
    {
      type: 'function',
      function: {
        name: 'list_files',
        description: 'List files in a directory',
        parameters: {
          type: 'object',
          properties: {
            path: { type: 'string', description: 'Directory path' },
          },
          required: ['path'],
        },
      },
    },
  ],
});

// Handle tool calls
if (response.choices[0].message.tool_calls) {
  for (const call of response.choices[0].message.tool_calls) {
    const args = JSON.parse(call.function.arguments);
    // Execute tool...
  }
}
```

## Environment Variables

```bash
# Required
export OPENROUTER_API_KEY=sk-or-...

# Optional
export OPENROUTER_BASE_URL=https://openrouter.ai/api/v1
export OPENROUTER_DEFAULT_MODEL=anthropic/claude-3.5-sonnet
export OPENROUTER_SITE_URL=https://your-app.com
export OPENROUTER_APP_NAME="Your App"
```

## Best Practices

1. **Always set fallbacks**: Primary models can be unavailable
2. **Track costs**: Monitor usage to avoid surprise bills
3. **Use appropriate models**: Don't use opus for simple tasks
4. **Handle rate limits**: Implement exponential backoff
5. **Cache responses**: For repeated queries, cache results
6. **Stream long responses**: Better UX for lengthy outputs
