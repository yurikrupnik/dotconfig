import OpenAI from 'openai';

// Input/Output types - customize for your skill
interface Input {
  prompt: string;
  context?: string;
  config?: {
    model?: string;
    maxTokens?: number;
    temperature?: number;
  };
}

interface Output {
  result: string;
  success: boolean;
  metadata?: {
    model: string;
    tokensUsed?: number;
    durationMs: number;
  };
}

interface ErrorOutput {
  success: false;
  error: {
    code: string;
    message: string;
  };
}

// OpenRouter client
const openai = new OpenAI({
  baseURL: 'https://openrouter.ai/api/v1',
  apiKey: process.env.OPENROUTER_API_KEY,
  defaultHeaders: {
    'HTTP-Referer': 'https://github.com/yurikrupnik/dotconfig',
    'X-Title': '{{skill_name}}',
  },
});

/**
 * Main skill execution - customize this function
 */
async function execute(input: Input): Promise<Output> {
  const start = Date.now();

  const model = input.config?.model || 'anthropic/claude-3-haiku';
  const maxTokens = input.config?.maxTokens || 2048;
  const temperature = input.config?.temperature || 0.7;

  const messages: OpenAI.ChatCompletionMessageParam[] = [];

  // Add context if provided
  if (input.context) {
    messages.push({
      role: 'system',
      content: input.context,
    });
  }

  messages.push({
    role: 'user',
    content: input.prompt,
  });

  const response = await openai.chat.completions.create({
    model,
    messages,
    max_tokens: maxTokens,
    temperature,
  });

  const result = response.choices[0].message.content || '';
  const duration = Date.now() - start;

  return {
    result,
    success: true,
    metadata: {
      model,
      tokensUsed: response.usage?.total_tokens,
      durationMs: duration,
    },
  };
}

/**
 * Read input from stdin
 */
function readStdin(): Promise<string> {
  return new Promise((resolve, reject) => {
    let data = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => (data += chunk));
    process.stdin.on('end', () => resolve(data));
    process.stdin.on('error', reject);
  });
}

/**
 * Main entry point
 */
async function main() {
  try {
    const inputStr = await readStdin();
    const input: Input = JSON.parse(inputStr);

    const output = await execute(input);
    console.log(JSON.stringify(output));
  } catch (error) {
    const errorOutput: ErrorOutput = {
      success: false,
      error: {
        code: 'EXECUTION_ERROR',
        message: error instanceof Error ? error.message : String(error),
      },
    };
    console.error(JSON.stringify(errorOutput));
    process.exit(1);
  }
}

main();
