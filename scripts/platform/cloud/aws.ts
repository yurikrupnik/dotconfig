import {
  EC2Client,
  RunInstancesCommand,
  DescribeInstancesCommand,
  TerminateInstancesCommand,
} from '@aws-sdk/client-ec2';
import {
  S3Client,
  ListBucketsCommand,
  CreateBucketCommand,
  PutObjectCommand,
  GetObjectCommand,
  DeleteObjectCommand,
} from '@aws-sdk/client-s3';
import {
  BedrockRuntimeClient,
  InvokeModelCommand,
} from '@aws-sdk/client-bedrock-runtime';
import {
  SecretsManagerClient,
  GetSecretValueCommand,
  CreateSecretCommand,
  UpdateSecretCommand,
  ListSecretsCommand,
} from '@aws-sdk/client-secrets-manager';
import type {
  CloudProvider,
  Instance,
  InstanceConfig,
  Bucket,
  BucketOptions,
  AIOptions,
} from './types.js';

export class AWSProvider implements CloudProvider {
  name = 'aws' as const;

  private ec2: EC2Client;
  private s3: S3Client;
  private bedrock: BedrockRuntimeClient;
  private secretsManager: SecretsManagerClient;

  constructor(region: string = 'us-east-1') {
    this.ec2 = new EC2Client({ region });
    this.s3 = new S3Client({ region });
    this.bedrock = new BedrockRuntimeClient({ region });
    this.secretsManager = new SecretsManagerClient({ region });
  }

  compute = {
    listInstances: async (): Promise<Instance[]> => {
      const response = await this.ec2.send(new DescribeInstancesCommand({}));
      return (
        response.Reservations?.flatMap(
          (r) =>
            r.Instances?.map((i) => ({
              id: i.InstanceId!,
              name: i.Tags?.find((t) => t.Key === 'Name')?.Value || '',
              status: i.State?.Name || 'unknown',
              type: i.InstanceType || '',
              publicIp: i.PublicIpAddress,
              privateIp: i.PrivateIpAddress,
              zone: i.Placement?.AvailabilityZone,
            })) || []
        ) || []
      );
    },

    createInstance: async (config: InstanceConfig): Promise<Instance> => {
      const response = await this.ec2.send(
        new RunInstancesCommand({
          ImageId: config.image,
          InstanceType: config.type as any,
          MinCount: 1,
          MaxCount: 1,
          TagSpecifications: [
            {
              ResourceType: 'instance',
              Tags: [
                { Key: 'Name', Value: config.name },
                ...Object.entries(config.tags || {}).map(([Key, Value]) => ({
                  Key,
                  Value,
                })),
              ],
            },
          ],
        })
      );
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
  };

  storage = {
    listBuckets: async (): Promise<Bucket[]> => {
      const response = await this.s3.send(new ListBucketsCommand({}));
      return (
        response.Buckets?.map((b) => ({
          name: b.Name!,
          location: 'us-east-1', // Would need additional call for actual region
          createdAt: b.CreationDate?.toISOString(),
        })) || []
      );
    },

    createBucket: async (
      name: string,
      options?: BucketOptions
    ): Promise<Bucket> => {
      await this.s3.send(
        new CreateBucketCommand({
          Bucket: name,
          CreateBucketConfiguration: options?.location
            ? { LocationConstraint: options.location as any }
            : undefined,
        })
      );
      return { name, location: options?.location || 'us-east-1' };
    },

    uploadFile: async (
      bucket: string,
      key: string,
      data: Buffer
    ): Promise<void> => {
      await this.s3.send(
        new PutObjectCommand({
          Bucket: bucket,
          Key: key,
          Body: data,
        })
      );
    },

    downloadFile: async (bucket: string, key: string): Promise<Buffer> => {
      const response = await this.s3.send(
        new GetObjectCommand({
          Bucket: bucket,
          Key: key,
        })
      );
      return Buffer.from(await response.Body!.transformToByteArray());
    },

    deleteFile: async (bucket: string, key: string): Promise<void> => {
      await this.s3.send(
        new DeleteObjectCommand({
          Bucket: bucket,
          Key: key,
        })
      );
    },
  };

  ai = {
    invoke: async (
      model: string,
      prompt: string,
      options?: AIOptions
    ): Promise<string> => {
      const body = JSON.stringify({
        anthropic_version: 'bedrock-2023-05-31',
        max_tokens: options?.maxTokens || 4096,
        temperature: options?.temperature || 0.7,
        messages: [{ role: 'user', content: prompt }],
      });

      const response = await this.bedrock.send(
        new InvokeModelCommand({
          modelId: model,
          contentType: 'application/json',
          accept: 'application/json',
          body: new TextEncoder().encode(body),
        })
      );

      const result = JSON.parse(new TextDecoder().decode(response.body));
      return result.content[0].text;
    },
  };

  secrets = {
    get: async (name: string): Promise<string> => {
      const response = await this.secretsManager.send(
        new GetSecretValueCommand({
          SecretId: name,
        })
      );
      return response.SecretString!;
    },

    set: async (name: string, value: string): Promise<void> => {
      try {
        await this.secretsManager.send(
          new UpdateSecretCommand({
            SecretId: name,
            SecretString: value,
          })
        );
      } catch (e: any) {
        if (e.name === 'ResourceNotFoundException') {
          await this.secretsManager.send(
            new CreateSecretCommand({
              Name: name,
              SecretString: value,
            })
          );
        } else {
          throw e;
        }
      }
    },

    list: async (): Promise<string[]> => {
      const response = await this.secretsManager.send(
        new ListSecretsCommand({})
      );
      return response.SecretList?.map((s) => s.Name!) || [];
    },
  };
}
