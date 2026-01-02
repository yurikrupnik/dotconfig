import { CloudProvider } from './types.js';
import { AWSProvider } from './aws.js';
import { GCPProvider } from './gcp.js';
import { AzureProvider } from './azure.js';

export type ProviderName = 'aws' | 'gcp' | 'azure';

/**
 * Create a cloud provider instance
 */
export function createProvider(name: ProviderName): CloudProvider {
  switch (name) {
    case 'aws':
      return new AWSProvider(process.env.AWS_REGION || 'us-east-1');
    case 'gcp':
      return new GCPProvider(
        process.env.GCP_PROJECT!,
        process.env.GCP_LOCATION || 'us-central1'
      );
    case 'azure':
      return new AzureProvider({
        subscriptionId: process.env.AZURE_SUBSCRIPTION_ID!,
        resourceGroup: process.env.AZURE_RESOURCE_GROUP!,
        storageAccount: process.env.AZURE_STORAGE_ACCOUNT!,
        openaiEndpoint: process.env.AZURE_OPENAI_ENDPOINT!,
        keyVaultUrl: process.env.AZURE_KEYVAULT_URL!,
      });
    default:
      throw new Error(`Unknown provider: ${name}`);
  }
}

/**
 * Multi-cloud provider with fallback support
 */
export class MultiCloudProvider {
  private providers: Map<ProviderName, CloudProvider> = new Map();
  private primary: ProviderName;

  constructor(primary: ProviderName) {
    this.primary = primary;
    this.addProvider(primary);
  }

  /**
   * Add a provider to the pool
   */
  addProvider(name: ProviderName): this {
    if (!this.providers.has(name)) {
      try {
        this.providers.set(name, createProvider(name));
      } catch (e) {
        console.warn(`Failed to initialize ${name} provider:`, e);
      }
    }
    return this;
  }

  /**
   * Get a specific provider
   */
  get(name?: ProviderName): CloudProvider {
    const providerName = name || this.primary;
    const provider = this.providers.get(providerName);
    if (!provider) {
      throw new Error(`Provider ${providerName} not initialized`);
    }
    return provider;
  }

  /**
   * Invoke AI with automatic fallback
   */
  async invokeAI(
    prompt: string,
    options?: { model?: string; routing?: 'simple' | 'complex' | 'coding' }
  ): Promise<string> {
    const providerOrder = [this.primary, ...Array.from(this.providers.keys())];

    for (const name of providerOrder) {
      const provider = this.providers.get(name);
      if (!provider) continue;

      try {
        const model = options?.model || this.getDefaultModel(name, options?.routing);
        return await provider.ai.invoke(model, prompt);
      } catch (error) {
        console.warn(`Provider ${name} failed:`, error);
      }
    }

    throw new Error('All providers failed');
  }

  /**
   * Upload file with fallback
   */
  async uploadFile(bucket: string, key: string, data: Buffer): Promise<void> {
    const provider = this.get();
    await provider.storage.uploadFile(bucket, key, data);
  }

  /**
   * Get secret with fallback
   */
  async getSecret(name: string): Promise<string> {
    for (const [providerName, provider] of this.providers) {
      try {
        return await provider.secrets.get(name);
      } catch (error) {
        console.warn(`Secret ${name} not found in ${providerName}`);
      }
    }
    throw new Error(`Secret ${name} not found in any provider`);
  }

  private getDefaultModel(
    provider: ProviderName,
    routing?: 'simple' | 'complex' | 'coding'
  ): string {
    const models: Record<ProviderName, Record<string, string>> = {
      aws: {
        simple: 'anthropic.claude-3-haiku-20240307-v1:0',
        complex: 'anthropic.claude-3-sonnet-20240229-v1:0',
        coding: 'anthropic.claude-3-sonnet-20240229-v1:0',
      },
      gcp: {
        simple: 'gemini-1.5-flash',
        complex: 'gemini-1.5-pro',
        coding: 'gemini-1.5-pro',
      },
      azure: {
        simple: 'gpt-35-turbo',
        complex: 'gpt-4',
        coding: 'gpt-4',
      },
    };

    return models[provider][routing || 'complex'];
  }
}

export * from './types.js';
export { AWSProvider } from './aws.js';
export { GCPProvider } from './gcp.js';
export { AzureProvider } from './azure.js';
