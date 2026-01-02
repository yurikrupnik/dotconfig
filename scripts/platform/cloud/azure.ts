import { BlobServiceClient } from '@azure/storage-blob';
import { OpenAIClient, AzureKeyCredential } from '@azure/openai';
import { SecretClient } from '@azure/keyvault-secrets';
import { DefaultAzureCredential } from '@azure/identity';
import type {
  CloudProvider,
  Instance,
  InstanceConfig,
  Bucket,
  BucketOptions,
  AIOptions,
} from './types.js';

interface AzureConfig {
  subscriptionId: string;
  resourceGroup: string;
  storageAccount: string;
  openaiEndpoint: string;
  keyVaultUrl: string;
}

export class AzureProvider implements CloudProvider {
  name = 'azure' as const;

  private blobService: BlobServiceClient;
  private openai: OpenAIClient;
  private secretClient: SecretClient;
  private config: AzureConfig;
  private credential: DefaultAzureCredential;

  constructor(config: AzureConfig) {
    this.config = config;
    this.credential = new DefaultAzureCredential();

    this.blobService = new BlobServiceClient(
      `https://${config.storageAccount}.blob.core.windows.net`,
      this.credential
    );

    this.openai = new OpenAIClient(
      config.openaiEndpoint,
      new AzureKeyCredential(process.env.AZURE_OPENAI_KEY || '')
    );

    this.secretClient = new SecretClient(config.keyVaultUrl, this.credential);
  }

  compute = {
    listInstances: async (): Promise<Instance[]> => {
      // Using az CLI via exec for now
      // In production, use @azure/arm-compute
      const { execSync } = await import('child_process');
      try {
        const output = execSync(
          `az vm list --resource-group ${this.config.resourceGroup} -o json`,
          { encoding: 'utf-8' }
        );
        const vms = JSON.parse(output);
        return vms.map((vm: any) => ({
          id: vm.id,
          name: vm.name,
          status: vm.provisioningState?.toLowerCase() || 'unknown',
          type: vm.hardwareProfile?.vmSize || '',
          zone: vm.location,
        }));
      } catch (e) {
        console.warn('Failed to list Azure VMs:', e);
        return [];
      }
    },

    createInstance: async (config: InstanceConfig): Promise<Instance> => {
      const { execSync } = await import('child_process');
      execSync(
        `az vm create ` +
          `--resource-group ${this.config.resourceGroup} ` +
          `--name ${config.name} ` +
          `--image ${config.image} ` +
          `--size ${config.type} ` +
          `--generate-ssh-keys`,
        { encoding: 'utf-8' }
      );
      return {
        id: config.name,
        name: config.name,
        status: 'running',
        type: config.type,
      };
    },

    deleteInstance: async (id: string): Promise<void> => {
      const { execSync } = await import('child_process');
      execSync(
        `az vm delete ` +
          `--resource-group ${this.config.resourceGroup} ` +
          `--name ${id} ` +
          `--yes`,
        { encoding: 'utf-8' }
      );
    },
  };

  storage = {
    listBuckets: async (): Promise<Bucket[]> => {
      const containers: Bucket[] = [];
      for await (const container of this.blobService.listContainers()) {
        containers.push({
          name: container.name,
          location: 'azure',
        });
      }
      return containers;
    },

    createBucket: async (
      name: string,
      _options?: BucketOptions
    ): Promise<Bucket> => {
      await this.blobService.createContainer(name);
      return { name, location: 'azure' };
    },

    uploadFile: async (
      bucket: string,
      key: string,
      data: Buffer
    ): Promise<void> => {
      const containerClient = this.blobService.getContainerClient(bucket);
      const blobClient = containerClient.getBlockBlobClient(key);
      await blobClient.uploadData(data);
    },

    downloadFile: async (bucket: string, key: string): Promise<Buffer> => {
      const containerClient = this.blobService.getContainerClient(bucket);
      const blobClient = containerClient.getBlockBlobClient(key);
      const response = await blobClient.downloadToBuffer();
      return response;
    },

    deleteFile: async (bucket: string, key: string): Promise<void> => {
      const containerClient = this.blobService.getContainerClient(bucket);
      const blobClient = containerClient.getBlockBlobClient(key);
      await blobClient.delete();
    },
  };

  ai = {
    invoke: async (
      model: string,
      prompt: string,
      options?: AIOptions
    ): Promise<string> => {
      const response = await this.openai.getChatCompletions(
        model,
        [{ role: 'user', content: prompt }],
        {
          maxTokens: options?.maxTokens || 4096,
          temperature: options?.temperature || 0.7,
        }
      );
      return response.choices[0].message!.content!;
    },

    embed: async (text: string): Promise<number[]> => {
      const response = await this.openai.getEmbeddings(
        'text-embedding-ada-002',
        [text]
      );
      return response.data[0].embedding;
    },
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const secret = await this.secretClient.getSecret(name);
      return secret.value!;
    },

    set: async (name: string, value: string): Promise<void> => {
      await this.secretClient.setSecret(name, value);
    },

    list: async (): Promise<string[]> => {
      const secrets: string[] = [];
      for await (const secret of this.secretClient.listPropertiesOfSecrets()) {
        secrets.push(secret.name);
      }
      return secrets;
    },
  };
}
