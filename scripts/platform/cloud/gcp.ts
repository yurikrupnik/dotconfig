import { Storage } from '@google-cloud/storage';
import { VertexAI } from '@google-cloud/vertexai';
import { SecretManagerServiceClient } from '@google-cloud/secret-manager';
import type {
  CloudProvider,
  Instance,
  InstanceConfig,
  Bucket,
  BucketOptions,
  AIOptions,
} from './types.js';

export class GCPProvider implements CloudProvider {
  name = 'gcp' as const;

  private storage: Storage;
  private vertexai: VertexAI;
  private secretsClient: SecretManagerServiceClient;
  private project: string;
  private location: string;

  constructor(project: string, location: string = 'us-central1') {
    this.project = project;
    this.location = location;
    this.storage = new Storage({ projectId: project });
    this.vertexai = new VertexAI({ project, location });
    this.secretsClient = new SecretManagerServiceClient();
  }

  compute = {
    listInstances: async (): Promise<Instance[]> => {
      // Using gcloud CLI via exec for now
      // In production, use @google-cloud/compute
      const { execSync } = await import('child_process');
      try {
        const output = execSync(
          `gcloud compute instances list --project=${this.project} --format=json`,
          { encoding: 'utf-8' }
        );
        const instances = JSON.parse(output);
        return instances.map((i: any) => ({
          id: i.id,
          name: i.name,
          status: i.status.toLowerCase(),
          type: i.machineType.split('/').pop(),
          publicIp: i.networkInterfaces?.[0]?.accessConfigs?.[0]?.natIP,
          privateIp: i.networkInterfaces?.[0]?.networkIP,
          zone: i.zone.split('/').pop(),
        }));
      } catch (e) {
        console.warn('Failed to list GCP instances:', e);
        return [];
      }
    },

    createInstance: async (config: InstanceConfig): Promise<Instance> => {
      const { execSync } = await import('child_process');
      const zone = config.zone || `${this.location}-a`;
      execSync(
        `gcloud compute instances create ${config.name} ` +
          `--project=${this.project} ` +
          `--zone=${zone} ` +
          `--machine-type=${config.type} ` +
          `--image=${config.image}`,
        { encoding: 'utf-8' }
      );
      return {
        id: config.name,
        name: config.name,
        status: 'running',
        type: config.type,
        zone,
      };
    },

    deleteInstance: async (id: string): Promise<void> => {
      const { execSync } = await import('child_process');
      execSync(
        `gcloud compute instances delete ${id} --project=${this.project} --quiet`,
        { encoding: 'utf-8' }
      );
    },
  };

  storage = {
    listBuckets: async (): Promise<Bucket[]> => {
      const [buckets] = await this.storage.getBuckets();
      return buckets.map((b) => ({
        name: b.name!,
        location: b.metadata.location || 'US',
        createdAt: b.metadata.timeCreated,
      }));
    },

    createBucket: async (
      name: string,
      options?: BucketOptions
    ): Promise<Bucket> => {
      const [bucket] = await this.storage.createBucket(name, {
        location: options?.location || 'US',
        storageClass: options?.storageClass || 'STANDARD',
      });
      return {
        name: bucket.name!,
        location: options?.location || 'US',
      };
    },

    uploadFile: async (
      bucket: string,
      key: string,
      data: Buffer
    ): Promise<void> => {
      await this.storage.bucket(bucket).file(key).save(data);
    },

    downloadFile: async (bucket: string, key: string): Promise<Buffer> => {
      const [contents] = await this.storage.bucket(bucket).file(key).download();
      return contents;
    },

    deleteFile: async (bucket: string, key: string): Promise<void> => {
      await this.storage.bucket(bucket).file(key).delete();
    },
  };

  ai = {
    invoke: async (
      model: string,
      prompt: string,
      options?: AIOptions
    ): Promise<string> => {
      const generativeModel = this.vertexai.getGenerativeModel({
        model,
        generationConfig: {
          maxOutputTokens: options?.maxTokens || 4096,
          temperature: options?.temperature || 0.7,
          topP: options?.topP || 0.95,
        },
      });

      const result = await generativeModel.generateContent(prompt);
      const response = await result.response;
      return response.candidates![0].content.parts[0].text!;
    },

    embed: async (text: string): Promise<number[]> => {
      const model = this.vertexai.getGenerativeModel({
        model: 'text-embedding-004',
      });
      const result = await model.embedContent(text);
      return result.embedding.values;
    },
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const [version] = await this.secretsClient.accessSecretVersion({
        name: `projects/${this.project}/secrets/${name}/versions/latest`,
      });
      return version.payload!.data!.toString();
    },

    set: async (name: string, value: string): Promise<void> => {
      try {
        // Try to add a new version
        await this.secretsClient.addSecretVersion({
          parent: `projects/${this.project}/secrets/${name}`,
          payload: {
            data: Buffer.from(value),
          },
        });
      } catch (e: any) {
        if (e.code === 5) {
          // NOT_FOUND - create the secret first
          await this.secretsClient.createSecret({
            parent: `projects/${this.project}`,
            secretId: name,
            secret: {
              replication: {
                automatic: {},
              },
            },
          });
          await this.secretsClient.addSecretVersion({
            parent: `projects/${this.project}/secrets/${name}`,
            payload: {
              data: Buffer.from(value),
            },
          });
        } else {
          throw e;
        }
      }
    },

    list: async (): Promise<string[]> => {
      const [secrets] = await this.secretsClient.listSecrets({
        parent: `projects/${this.project}`,
      });
      return secrets.map((s) => s.name!.split('/').pop()!);
    },
  };
}
