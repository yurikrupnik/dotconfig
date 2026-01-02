#!/usr/bin/env tsx
/**
 * Cloud CLI - TypeScript wrapper for Nu commands
 */

import { createProvider, MultiCloudProvider, ProviderName } from './providers.js';

const args = process.argv.slice(2);
const command = args[0];

async function main() {
  switch (command) {
    case 'ai-invoke':
      await aiInvoke(args.slice(1));
      break;
    case 'storage-upload':
      await storageUpload(args.slice(1));
      break;
    case 'storage-download':
      await storageDownload(args.slice(1));
      break;
    case 'secret-get':
      await secretGet(args.slice(1));
      break;
    case 'instances-list':
      await instancesList(args.slice(1));
      break;
    default:
      console.error(`Unknown command: ${command}`);
      printUsage();
      process.exit(1);
  }
}

function parseArgs(args: string[]): Record<string, string> {
  const result: Record<string, string> = {};
  for (let i = 0; i < args.length; i++) {
    if (args[i].startsWith('--')) {
      const key = args[i].slice(2);
      const value = args[i + 1] || '';
      result[key] = value;
      i++;
    }
  }
  return result;
}

async function aiInvoke(args: string[]) {
  const opts = parseArgs(args);
  const provider = (opts.provider || 'gcp') as ProviderName;
  const model = opts.model || '';
  const prompt = opts.prompt || '';

  if (!prompt) {
    console.error('Missing --prompt');
    process.exit(1);
  }

  const cloud = createProvider(provider);
  const result = await cloud.ai.invoke(model, prompt);
  console.log(result);
}

async function storageUpload(args: string[]) {
  const opts = parseArgs(args);
  const provider = (opts.provider || 'gcp') as ProviderName;
  const bucket = opts.bucket;
  const key = opts.key;
  const file = opts.file;

  if (!bucket || !key || !file) {
    console.error('Missing required arguments: --bucket, --key, --file');
    process.exit(1);
  }

  const fs = await import('fs');
  const data = fs.readFileSync(file);

  const cloud = createProvider(provider);
  await cloud.storage.uploadFile(bucket, key, data);
  console.log(`Uploaded ${file} to ${provider}://${bucket}/${key}`);
}

async function storageDownload(args: string[]) {
  const opts = parseArgs(args);
  const provider = (opts.provider || 'gcp') as ProviderName;
  const bucket = opts.bucket;
  const key = opts.key;
  const file = opts.file;

  if (!bucket || !key || !file) {
    console.error('Missing required arguments: --bucket, --key, --file');
    process.exit(1);
  }

  const fs = await import('fs');
  const cloud = createProvider(provider);
  const data = await cloud.storage.downloadFile(bucket, key);
  fs.writeFileSync(file, data);
  console.log(`Downloaded ${provider}://${bucket}/${key} to ${file}`);
}

async function secretGet(args: string[]) {
  const opts = parseArgs(args);
  const provider = (opts.provider || 'gcp') as ProviderName;
  const name = opts.name;

  if (!name) {
    console.error('Missing --name');
    process.exit(1);
  }

  const cloud = createProvider(provider);
  const value = await cloud.secrets.get(name);
  console.log(value);
}

async function instancesList(args: string[]) {
  const opts = parseArgs(args);
  const provider = (opts.provider || 'gcp') as ProviderName;

  const cloud = createProvider(provider);
  const instances = await cloud.compute.listInstances();
  console.log(JSON.stringify(instances, null, 2));
}

function printUsage() {
  console.log(`
Cloud CLI - Unified cloud provider interface

Commands:
  ai-invoke --provider <aws|gcp|azure> --model <model> --prompt <prompt>
  storage-upload --provider <p> --bucket <b> --key <k> --file <f>
  storage-download --provider <p> --bucket <b> --key <k> --file <f>
  secret-get --provider <p> --name <n>
  instances-list --provider <p>

Environment variables:
  AWS: AWS_REGION, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
  GCP: GCP_PROJECT, GCP_LOCATION, GOOGLE_APPLICATION_CREDENTIALS
  Azure: AZURE_SUBSCRIPTION_ID, AZURE_RESOURCE_GROUP, AZURE_STORAGE_ACCOUNT,
         AZURE_OPENAI_ENDPOINT, AZURE_OPENAI_KEY, AZURE_KEYVAULT_URL
`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
