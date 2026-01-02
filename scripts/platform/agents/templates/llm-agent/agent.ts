import { DaprClient, DaprServer } from '@dapr/dapr';
import OpenAI from 'openai';
import express from 'express';

// Configuration
const AGENT_ID = process.env.AGENT_ID || '{{agent_name}}';
const AGENT_MODEL = process.env.AGENT_MODEL || 'anthropic/claude-3.5-sonnet';
const APP_PORT = parseInt(process.env.APP_PORT || '3001');
const DAPR_HTTP_PORT = parseInt(process.env.DAPR_HTTP_PORT || '3500');

// Types
interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string;
  name?: string;
  tool_call_id?: string;
}

interface Tool {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: object;
  };
}

interface AgentState {
  context: Message[];
  metadata: {
    createdAt: string;
    lastActiveAt: string;
    totalTokens: number;
  };
}

/**
 * LLM Agent with Dapr integration
 */
class Agent {
  private dapr: DaprClient;
  private server: DaprServer;
  private app: express.Application;
  private openai: OpenAI;
  private instanceId: string;
  private model: string;
  private context: Message[] = [];
  private tools: Tool[] = [];
  private systemPrompt: string;

  constructor() {
    this.instanceId = AGENT_ID;
    this.model = AGENT_MODEL;
    this.systemPrompt = this.loadSystemPrompt();

    this.dapr = new DaprClient();
    this.server = new DaprServer();
    this.app = express();
    this.app.use(express.json());

    this.openai = new OpenAI({
      baseURL: 'https://openrouter.ai/api/v1',
      apiKey: process.env.OPENROUTER_API_KEY,
      defaultHeaders: {
        'HTTP-Referer': 'https://github.com/yurikrupnik/dotconfig',
        'X-Title': `Agent: ${this.instanceId}`,
      },
    });

    this.setupRoutes();
    this.setupTools();
  }

  /**
   * Load system prompt from config or use default
   */
  private loadSystemPrompt(): string {
    return process.env.AGENT_SYSTEM_PROMPT || `
You are a helpful AI assistant named ${AGENT_ID}.
You have access to tools that allow you to perform various tasks.
Always think step by step and use tools when appropriate.
Be concise but thorough in your responses.
    `.trim();
  }

  /**
   * Setup available tools - customize for your agent
   */
  private setupTools(): void {
    this.tools = [
      {
        type: 'function',
        function: {
          name: 'search_files',
          description: 'Search for files matching a pattern',
          parameters: {
            type: 'object',
            properties: {
              pattern: { type: 'string', description: 'Glob pattern to match' },
              path: { type: 'string', description: 'Directory to search in' },
            },
            required: ['pattern'],
          },
        },
      },
      {
        type: 'function',
        function: {
          name: 'read_file',
          description: 'Read contents of a file',
          parameters: {
            type: 'object',
            properties: {
              path: { type: 'string', description: 'File path to read' },
            },
            required: ['path'],
          },
        },
      },
    ];
  }

  /**
   * Setup Express routes
   */
  private setupRoutes(): void {
    // Health check
    this.app.get('/health', (_, res) => {
      res.json({ status: 'healthy', instanceId: this.instanceId });
    });

    // Get agent status
    this.app.get('/status', (_, res) => {
      res.json({
        instanceId: this.instanceId,
        model: this.model,
        contextLength: this.context.length,
        status: 'running',
      });
    });

    // Invoke agent
    this.app.post('/invoke', async (req, res) => {
      try {
        const { prompt, stream } = req.body;
        const response = await this.invoke(prompt, stream);
        res.json({ response });
      } catch (error) {
        res.status(500).json({
          error: error instanceof Error ? error.message : String(error),
        });
      }
    });

    // Graceful shutdown
    this.app.post('/shutdown', async (_, res) => {
      await this.shutdown();
      res.json({ status: 'shutting_down' });
      setTimeout(() => process.exit(0), 1000);
    });

    // Clear context
    this.app.post('/clear', async (_, res) => {
      this.context = [];
      await this.saveState();
      res.json({ status: 'context_cleared' });
    });
  }

  /**
   * Start the agent
   */
  async start(): Promise<void> {
    // Load persisted state
    await this.loadState();

    // Initialize context with system prompt if empty
    if (this.context.length === 0) {
      this.context.push({
        role: 'system',
        content: this.systemPrompt,
      });
    }

    // Start Dapr subscriptions
    await this.setupSubscriptions();

    // Start Express server
    this.app.listen(APP_PORT, () => {
      console.log(`Agent ${this.instanceId} started on port ${APP_PORT}`);
      console.log(`Model: ${this.model}`);
      console.log(`Dapr HTTP port: ${DAPR_HTTP_PORT}`);
    });
  }

  /**
   * Setup Dapr pub/sub subscriptions
   */
  private async setupSubscriptions(): Promise<void> {
    try {
      // Subscribe to agent messages
      this.server.pubsub.subscribe('events', 'agent.message', async (data: any) => {
        if (data.target === this.instanceId) {
          console.log(`Received message from ${data.from}`);
          const response = await this.invoke(data.content);

          // Reply to sender
          await this.dapr.pubsub.publish('events', 'agent.message', {
            from: this.instanceId,
            target: data.from,
            content: response,
            timestamp: new Date().toISOString(),
          });
        }
      });

      await this.server.start();
    } catch (error) {
      console.warn('Dapr not available, running in standalone mode');
    }
  }

  /**
   * Invoke the agent with a prompt
   */
  async invoke(prompt: string, stream = false): Promise<string> {
    // Add a user message
    this.context.push({ role: 'user', content: prompt });

    // Call LLM
    const response = await this.openai.chat.completions.create({
      model: this.model,
      messages: this.context,
      tools: this.tools.length > 0 ? this.tools : undefined,
      max_tokens: 4096,
      temperature: 0.7,
    });

    const message = response.choices[0].message;

    // Handle tool calls
    if (message.tool_calls && message.tool_calls.length > 0) {
      // Add an assistant message with tool calls
      this.context.push({
        role: 'assistant',
        content: message.content || '',
      });

      for (const toolCall of message.tool_calls) {
        const result = await this.executeTool(
          toolCall.function.name,
          JSON.parse(toolCall.function.arguments)
        );

        this.context.push({
          role: 'tool',
          content: JSON.stringify(result),
          tool_call_id: toolCall.id,
          name: toolCall.function.name,
        });
      }

      // Get a final response after tool execution
      return this.invoke('');
    }

    // Add assistant response to context
    const reply = message.content || '';
    this.context.push({ role: 'assistant', content: reply });

    // Persist state
    await this.saveState();

    // Publish action event
    try {
      await this.dapr.pubsub.publish('events', 'agent.action', {
        agentId: this.instanceId,
        action: 'invoke',
        timestamp: new Date().toISOString(),
      });
    } catch {
      // Dapr not available
    }

    return reply;
  }

  /**
   * Execute a tool - customize for your agent
   */
  private async executeTool(name: string, args: Record<string, any>): Promise<any> {
    console.log(`Executing tool: ${name}`, args);

    switch (name) {
      case 'search_files':
        // Invoke skill via Dapr or directly
        try {
          return await this.dapr.invoker.invoke(
            'file-search-skill',
            'search',
            { method: 'POST', body: args }
          );
        } catch {
          return { error: 'Skill not available', files: [] };
        }

      case 'read_file':
        const fs = await import('fs/promises');
        try {
          const content = await fs.readFile(args.path, 'utf-8');
          return { content, path: args.path };
        } catch (e) {
          return { error: `Failed to read file: ${e}` };
        }

      default:
        return { error: `Unknown tool: ${name}` };
    }
  }

  /**
   * Load state from the Dapr state store
   */
  private async loadState(): Promise<void> {
    try {
      const state = await this.dapr.state.get('statestore', `agent:${this.instanceId}`);
      if (state) {
        const parsed = state as AgentState;
        this.context = parsed.context || [];
        console.log(`Loaded state with ${this.context.length} messages`);
      }
    } catch (error) {
      console.log('No previous state found, starting fresh');
    }
  }

  /**
   * Save state to Dapr state store
   */
  private async saveState(): Promise<void> {
    try {
      const state: AgentState = {
        context: this.context,
        metadata: {
          createdAt: new Date().toISOString(),
          lastActiveAt: new Date().toISOString(),
          totalTokens: 0, // Would need to track this
        },
      };

      await this.dapr.state.save('statestore', [
        { key: `agent:${this.instanceId}`, value: state },
      ]);
    } catch (error) {
      console.warn('Failed to save state:', error);
    }
  }

  /**
   * Graceful shutdown
   */
  private async shutdown(): Promise<void> {
    console.log('Agent shutting down...');
    await this.saveState();

    try {
      await this.dapr.pubsub.publish('events', 'agent.stopped', {
        agentId: this.instanceId,
        timestamp: new Date().toISOString(),
      });
    } catch {
      // Dapr not available
    }
  }
}

// Start agent
const agent = new Agent();
agent.start().catch((error) => {
  console.error('Failed to start agent:', error);
  process.exit(1);
});
