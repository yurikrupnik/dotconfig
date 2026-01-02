# Cloud Providers API Reference

## Overview

The platform provides a unified interface to major cloud providers:

- **AWS** (Amazon Web Services)
- **GCP** (Google Cloud Platform)
- **Azure** (Microsoft Azure)

## Unified Interface

```typescript
interface CloudProvider {
  // Identity
  name: 'aws' | 'gcp' | 'azure';

  // Compute
  compute: {
    listInstances(): Promise<Instance[]>;
    createInstance(config: InstanceConfig): Promise<Instance>;
    deleteInstance(id: string): Promise<void>;
    runContainer(config: ContainerConfig): Promise<Container>;
  };

  // Storage
  storage: {
    listBuckets(): Promise<Bucket[]>;
    createBucket(name: string, options?: BucketOptions): Promise<Bucket>;
    uploadFile(bucket: string, key: string, data: Buffer): Promise<void>;
    downloadFile(bucket: string, key: string): Promise<Buffer>;
    deleteFile(bucket: string, key: string): Promise<void>;
  };

  // AI/ML
  ai: {
    invoke(model: string, prompt: string, options?: AIOptions): Promise<string>;
    embed(text: string): Promise<number[]>;
    transcribe(audio: Buffer): Promise<string>;
  };

  // Secrets
  secrets: {
    get(name: string): Promise<string>;
    set(name: string, value: string): Promise<void>;
    list(): Promise<string[]>;
  };

  // Databases
  database: {
    query(sql: string, params?: any[]): Promise<any[]>;
    execute(sql: string, params?: any[]): Promise<void>;
  };
}
```

## AWS Implementation

**cloud/aws.ts:**
```typescript
import {
  EC2Client,
  RunInstancesCommand,
  DescribeInstancesCommand,
  TerminateInstancesCommand
} from '@aws-sdk/client-ec2';
import { S3Client, PutObjectCommand, GetObjectCommand } from '@aws-sdk/client-s3';
import { BedrockRuntimeClient, InvokeModelCommand } from '@aws-sdk/client-bedrock-runtime';
import { SecretsManagerClient, GetSecretValueCommand } from '@aws-sdk/client-secrets-manager';

export class AWSProvider implements CloudProvider {
  name = 'aws' as const;

  private ec2: EC2Client;
  private s3: S3Client;
  private bedrock: BedrockRuntimeClient;
  private secrets: SecretsManagerClient;

  constructor(region: string = 'us-east-1') {
    this.ec2 = new EC2Client({ region });
    this.s3 = new S3Client({ region });
    this.bedrock = new BedrockRuntimeClient({ region });
    this.secrets = new SecretsManagerClient({ region });
  }

  compute = {
    listInstances: async (): Promise<Instance[]> => {
      const response = await this.ec2.send(new DescribeInstancesCommand({}));
      return response.Reservations?.flatMap(r =>
        r.Instances?.map(i => ({
          id: i.InstanceId!,
          name: i.Tags?.find(t => t.Key === 'Name')?.Value || '',
          status: i.State?.Name || 'unknown',
          type: i.InstanceType || '',
          publicIp: i.PublicIpAddress,
        })) || []
      ) || [];
    },

    createInstance: async (config: InstanceConfig): Promise<Instance> => {
      const response = await this.ec2.send(new RunInstancesCommand({
        ImageId: config.image,
        InstanceType: config.type,
        MinCount: 1,
        MaxCount: 1,
        TagSpecifications: [{
          ResourceType: 'instance',
          Tags: [{ Key: 'Name', Value: config.name }],
        }],
      }));
      const instance = response.Instances![0];
      return {
        id: instance.InstanceId!,
        name: config.name,
        status: 'pending',
        type: config.type,
      };
    },

    deleteInstance: async (id: string): Promise<void> => {
      await this.ec2.send(new TerminateInstancesCommand({ InstanceIds: [id] }));
    },

    runContainer: async (config: ContainerConfig): Promise<Container> => {
      // Use ECS or Fargate
      // Implementation...
    },
  };

  storage = {
    uploadFile: async (bucket: string, key: string, data: Buffer): Promise<void> => {
      await this.s3.send(new PutObjectCommand({
        Bucket: bucket,
        Key: key,
        Body: data,
      }));
    },

    downloadFile: async (bucket: string, key: string): Promise<Buffer> => {
      const response = await this.s3.send(new GetObjectCommand({
        Bucket: bucket,
        Key: key,
      }));
      return Buffer.from(await response.Body!.transformToByteArray());
    },
    // ... more methods
  };

  ai = {
    invoke: async (model: string, prompt: string): Promise<string> => {
      const response = await this.bedrock.send(new InvokeModelCommand({
        modelId: model, // e.g., 'anthropic.claude-3-sonnet-20240229-v1:0'
        body: JSON.stringify({
          anthropic_version: 'bedrock-2023-05-31',
          max_tokens: 4096,
          messages: [{ role: 'user', content: prompt }],
        }),
      }));
      const result = JSON.parse(new TextDecoder().decode(response.body));
      return result.content[0].text;
    },
    // ... more methods
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const response = await this.secrets.send(new GetSecretValueCommand({
        SecretId: name,
      }));
      return response.SecretString!;
    },
    // ... more methods
  };
}
```

## GCP Implementation

**cloud/gcp.ts:**
```typescript
import { Compute } from '@google-cloud/compute';
import { Storage } from '@google-cloud/storage';
import { VertexAI } from '@google-cloud/vertexai';
import { SecretManagerServiceClient } from '@google-cloud/secret-manager';

export class GCPProvider implements CloudProvider {
  name = 'gcp' as const;

  private compute: Compute;
  private storage: Storage;
  private vertexai: VertexAI;
  private secrets: SecretManagerServiceClient;
  private project: string;

  constructor(project: string, location: string = 'us-central1') {
    this.project = project;
    this.compute = new Compute({ projectId: project });
    this.storage = new Storage({ projectId: project });
    this.vertexai = new VertexAI({ project, location });
    this.secrets = new SecretManagerServiceClient();
  }

  compute = {
    listInstances: async (): Promise<Instance[]> => {
      const [vms] = await this.compute.getVMs();
      return vms.map(vm => ({
        id: vm.id!,
        name: vm.name!,
        status: vm.metadata.status,
        type: vm.metadata.machineType.split('/').pop()!,
        publicIp: vm.metadata.networkInterfaces?.[0]?.accessConfigs?.[0]?.natIP,
      }));
    },
    // ... more methods
  };

  storage = {
    uploadFile: async (bucket: string, key: string, data: Buffer): Promise<void> => {
      await this.storage.bucket(bucket).file(key).save(data);
    },

    downloadFile: async (bucket: string, key: string): Promise<Buffer> => {
      const [contents] = await this.storage.bucket(bucket).file(key).download();
      return contents;
    },
    // ... more methods
  };

  ai = {
    invoke: async (model: string, prompt: string): Promise<string> => {
      const generativeModel = this.vertexai.getGenerativeModel({ model });
      const result = await generativeModel.generateContent(prompt);
      return result.response.candidates![0].content.parts[0].text!;
    },
    // ... more methods
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const [version] = await this.secrets.accessSecretVersion({
        name: `projects/${this.project}/secrets/${name}/versions/latest`,
      });
      return version.payload!.data!.toString();
    },
    // ... more methods
  };
}
```

## Azure Implementation

**cloud/azure.ts:**
```typescript
import { ComputeManagementClient } from '@azure/arm-compute';
import { BlobServiceClient } from '@azure/storage-blob';
import { OpenAIClient, AzureKeyCredential } from '@azure/openai';
import { SecretClient } from '@azure/keyvault-secrets';
import { DefaultAzureCredential } from '@azure/identity';

export class AzureProvider implements CloudProvider {
  name = 'azure' as const;

  private compute: ComputeManagementClient;
  private storage: BlobServiceClient;
  private openai: OpenAIClient;
  private secrets: SecretClient;

  constructor(
    subscriptionId: string,
    resourceGroup: string,
    storageAccount: string,
    openaiEndpoint: string,
    keyVaultUrl: string
  ) {
    const credential = new DefaultAzureCredential();
    this.compute = new ComputeManagementClient(credential, subscriptionId);
    this.storage = new BlobServiceClient(
      `https://${storageAccount}.blob.core.windows.net`,
      credential
    );
    this.openai = new OpenAIClient(openaiEndpoint, new AzureKeyCredential(process.env.AZURE_OPENAI_KEY!));
    this.secrets = new SecretClient(keyVaultUrl, credential);
  }

  ai = {
    invoke: async (model: string, prompt: string): Promise<string> => {
      const response = await this.openai.getChatCompletions(model, [
        { role: 'user', content: prompt },
      ]);
      return response.choices[0].message!.content!;
    },
    // ... more methods
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const secret = await this.secrets.getSecret(name);
      return secret.value!;
    },
    // ... more methods
  };

  // ... more implementations
}
```

## Provider Factory

**cloud/providers.ts:**
```typescript
import { AWSProvider } from './aws';
import { GCPProvider } from './gcp';
import { AzureProvider } from './azure';

export function createProvider(name: 'aws' | 'gcp' | 'azure'): CloudProvider {
  switch (name) {
    case 'aws':
      return new AWSProvider(process.env.AWS_REGION);
    case 'gcp':
      return new GCPProvider(process.env.GCP_PROJECT!, process.env.GCP_LOCATION);
    case 'azure':
      return new AzureProvider(
        process.env.AZURE_SUBSCRIPTION_ID!,
        process.env.AZURE_RESOURCE_GROUP!,
        process.env.AZURE_STORAGE_ACCOUNT!,
        process.env.AZURE_OPENAI_ENDPOINT!,
        process.env.AZURE_KEYVAULT_URL!
      );
  }
}

// Multi-cloud operations
export class MultiCloudProvider {
  private providers: Map<string, CloudProvider>;
  private primary: string;

  constructor(primary: 'aws' | 'gcp' | 'azure') {
    this.primary = primary;
    this.providers = new Map();
    this.providers.set(primary, createProvider(primary));
  }

  addProvider(name: 'aws' | 'gcp' | 'azure'): void {
    if (!this.providers.has(name)) {
      this.providers.set(name, createProvider(name));
    }
  }

  get(name?: string): CloudProvider {
    const providerName = name || this.primary;
    return this.providers.get(providerName)!;
  }

  // Unified operations with fallback
  async invoke(prompt: string): Promise<string> {
    for (const [name, provider] of this.providers) {
      try {
        return await provider.ai.invoke('default', prompt);
      } catch (error) {
        console.warn(`Provider ${name} failed, trying next...`);
      }
    }
    throw new Error('All providers failed');
  }
}
```

## Nu Commands

**commands/cloud.nu:**
```nushell
# List available providers
export def "main cloud list-providers" [] {
  ['aws', 'gcp', 'azure'] | each { |p|
    { name: $p, configured: (check-provider $p) }
  }
}

# Set active provider
export def "main cloud use" [provider: string] {
  $env.CLOUD_PROVIDER = $provider
  print $"Active provider set to: ($provider)"
}

# Invoke AI across providers
export def "main cloud ai invoke" [
  model: string
  prompt: string
  --provider: string = ""
] {
  let p = if $provider == "" { $env.CLOUD_PROVIDER? | default "gcp" } else { $provider }

  # Call TypeScript wrapper
  node scripts/platform/cloud/cli.js ai-invoke --provider $p --model $model --prompt $prompt
}
```

## Environment Setup

```bash
# AWS
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
export AWS_REGION=us-east-1

# GCP
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
export GCP_PROJECT=my-project
export GCP_LOCATION=us-central1

# Azure
export AZURE_SUBSCRIPTION_ID=...
export AZURE_TENANT_ID=...
export AZURE_CLIENT_ID=...
export AZURE_CLIENT_SECRET=...
export AZURE_RESOURCE_GROUP=my-rg
export AZURE_STORAGE_ACCOUNT=mystorageaccount
export AZURE_OPENAI_ENDPOINT=https://my-openai.openai.azure.com
export AZURE_KEYVAULT_URL=https://my-vault.vault.azure.net
```
