// Unified Cloud Provider Types

export interface Instance {
  id: string;
  name: string;
  status: string;
  type: string;
  publicIp?: string;
  privateIp?: string;
  zone?: string;
  createdAt?: string;
}

export interface InstanceConfig {
  name: string;
  type: string;
  image: string;
  zone?: string;
  tags?: Record<string, string>;
}

export interface Bucket {
  name: string;
  location: string;
  createdAt?: string;
}

export interface BucketOptions {
  location?: string;
  storageClass?: string;
}

export interface AIOptions {
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stopSequences?: string[];
}

export interface CloudProvider {
  name: 'aws' | 'gcp' | 'azure';

  compute: {
    listInstances(): Promise<Instance[]>;
    createInstance(config: InstanceConfig): Promise<Instance>;
    deleteInstance(id: string): Promise<void>;
  };

  storage: {
    listBuckets(): Promise<Bucket[]>;
    createBucket(name: string, options?: BucketOptions): Promise<Bucket>;
    uploadFile(bucket: string, key: string, data: Buffer): Promise<void>;
    downloadFile(bucket: string, key: string): Promise<Buffer>;
    deleteFile(bucket: string, key: string): Promise<void>;
  };

  ai: {
    invoke(model: string, prompt: string, options?: AIOptions): Promise<string>;
    embed?(text: string): Promise<number[]>;
  };

  secrets: {
    get(name: string): Promise<string>;
    set(name: string, value: string): Promise<void>;
    list(): Promise<string[]>;
  };
}
