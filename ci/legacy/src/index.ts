/**
 * Dagger CI Pipeline for dotconfig
 *
 * Builds both binaries:
 * - dotconfig: Main CLI tool
 * - operator: Kubernetes node metrics operator
 */

import { connect, Client, Container } from '@dagger.io/dagger';

// Configuration
const RUST_VERSION = '1.83';
const BINARIES = ['dotconfig', 'operator'] as const;
type Binary = (typeof BINARIES)[number];

interface BuildResult {
  binary: string;
  size: string;
  success: boolean;
}

interface TestResult {
  passed: boolean;
  output: string;
}

/**
 * Get Rust base container with caching
 */
function getRustContainer(client: Client): Container {
  const cargoCache = client.cacheVolume('cargo-cache');
  const targetCache = client.cacheVolume('rust-target-cache');

  return client
    .container()
    .from(`rust:${RUST_VERSION}-slim`)
    .withExec(['apt-get', 'update'])
    .withExec([
      'apt-get',
      'install',
      '-y',
      'pkg-config',
      'libssl-dev',
      'musl-tools',
    ])
    .withMountedCache('/usr/local/cargo/registry', cargoCache)
    .withMountedCache('/app/target', targetCache)
    .withEnvVariable('CARGO_HOME', '/usr/local/cargo')
    .withEnvVariable('CARGO_INCREMENTAL', '0');
}

/**
 * Build a specific binary
 */
async function buildBinary(
  client: Client,
  source: ReturnType<Client['host']>['directory'],
  binary: Binary,
  release: boolean = true
): Promise<Container> {
  const container = getRustContainer(client);
  const profile = release ? '--release' : '';
  const targetDir = release ? 'release' : 'debug';

  return container
    .withDirectory('/app', source)
    .withWorkdir('/app')
    .withExec(['cargo', 'build', '--bin', binary, ...(release ? ['--release'] : [])])
    .withExec(['ls', '-la', `/app/target/${targetDir}/${binary}`]);
}

/**
 * Build all binaries
 */
async function build(client: Client): Promise<BuildResult[]> {
  console.log('🔨 Building binaries...');

  const source = client.host().directory('.', {
    exclude: ['target', 'ci/node_modules', '.git'],
  });

  const results: BuildResult[] = [];

  for (const binary of BINARIES) {
    console.log(`  Building ${binary}...`);
    try {
      const container = await buildBinary(client, source, binary);
      const output = await container.stdout();

      // Get binary size
      const sizeOutput = await container
        .withExec(['ls', '-lh', `/app/target/release/${binary}`])
        .stdout();

      const sizeMatch = sizeOutput.match(/(\d+[KMG]?)\s/);
      const size = sizeMatch ? sizeMatch[1] : 'unknown';

      results.push({ binary, size, success: true });
      console.log(`  ✅ ${binary} built (${size})`);
    } catch (error) {
      results.push({ binary, size: '0', success: false });
      console.error(`  ❌ ${binary} failed:`, error);
    }
  }

  return results;
}

/**
 * Run tests
 */
async function test(client: Client): Promise<TestResult> {
  console.log('🧪 Running tests...');

  const source = client.host().directory('.', {
    exclude: ['target', 'ci/node_modules', '.git'],
  });

  const container = getRustContainer(client);

  try {
    const output = await container
      .withDirectory('/app', source)
      .withWorkdir('/app')
      .withExec(['cargo', 'test', '--all'])
      .stdout();

    console.log('✅ Tests passed');
    return { passed: true, output };
  } catch (error) {
    console.error('❌ Tests failed:', error);
    return { passed: false, output: String(error) };
  }
}

/**
 * Run clippy lint
 */
async function lint(client: Client): Promise<boolean> {
  console.log('🔍 Running clippy...');

  const source = client.host().directory('.', {
    exclude: ['target', 'ci/node_modules', '.git'],
  });

  const container = getRustContainer(client);

  try {
    await container
      .withDirectory('/app', source)
      .withWorkdir('/app')
      .withExec(['rustup', 'component', 'add', 'clippy'])
      .withExec(['cargo', 'clippy', '--all', '--', '-D', 'warnings'])
      .stdout();

    console.log('✅ Clippy passed');
    return true;
  } catch (error) {
    console.error('❌ Clippy failed:', error);
    return false;
  }
}

/**
 * Build container images
 */
async function buildContainers(
  client: Client,
  registry?: string
): Promise<Map<Binary, Container>> {
  console.log('🐳 Building container images...');

  const source = client.host().directory('.', {
    exclude: ['target', 'ci/node_modules', '.git'],
  });

  const containers = new Map<Binary, Container>();

  // Build binaries first (statically linked with musl)
  const builder = getRustContainer(client)
    .withExec(['rustup', 'target', 'add', 'x86_64-unknown-linux-musl'])
    .withDirectory('/app', source)
    .withWorkdir('/app');

  for (const binary of BINARIES) {
    console.log(`  Building container for ${binary}...`);

    // Build static binary
    const builtBinary = await builder
      .withExec([
        'cargo',
        'build',
        '--bin',
        binary,
        '--release',
        '--target',
        'x86_64-unknown-linux-musl',
      ])
      .file(`/app/target/x86_64-unknown-linux-musl/release/${binary}`);

    // Create minimal container
    const container = client
      .container()
      .from('gcr.io/distroless/static-debian12:nonroot')
      .withFile(`/app/${binary}`, builtBinary)
      .withEntrypoint([`/app/${binary}`])
      .withLabel('org.opencontainers.image.source', 'https://github.com/yurikrupnik/dotconfig')
      .withLabel('org.opencontainers.image.title', binary);

    containers.set(binary, container);
    console.log(`  ✅ Container for ${binary} built`);
  }

  return containers;
}

/**
 * Publish containers to registry
 */
async function publishContainers(
  client: Client,
  registry: string,
  tag: string = 'latest'
): Promise<void> {
  console.log(`📤 Publishing to ${registry}...`);

  const containers = await buildContainers(client);

  for (const [binary, container] of containers) {
    const imageRef = `${registry}/${binary}:${tag}`;
    console.log(`  Publishing ${imageRef}...`);

    try {
      const digest = await container.publish(imageRef);
      console.log(`  ✅ Published: ${digest}`);
    } catch (error) {
      console.error(`  ❌ Failed to publish ${binary}:`, error);
      throw error;
    }
  }
}

/**
 * Run full CI pipeline
 */
async function runAll(client: Client): Promise<boolean> {
  console.log('🚀 Running full CI pipeline...\n');

  // Run lint and test in parallel concept (sequential here for simplicity)
  const lintPassed = await lint(client);
  const testResult = await test(client);
  const buildResults = await build(client);

  console.log('\n📊 Summary:');
  console.log(`  Lint: ${lintPassed ? '✅' : '❌'}`);
  console.log(`  Test: ${testResult.passed ? '✅' : '❌'}`);
  console.log(`  Build:`);
  for (const result of buildResults) {
    console.log(`    ${result.binary}: ${result.success ? '✅' : '❌'} (${result.size})`);
  }

  const allPassed =
    lintPassed && testResult.passed && buildResults.every((r) => r.success);

  console.log(`\n${allPassed ? '✅ Pipeline passed!' : '❌ Pipeline failed!'}`);
  return allPassed;
}

/**
 * Main entry point
 */
async function main(): Promise<void> {
  const command = process.argv[2] || 'all';

  await connect(
    async (client: Client) => {
      switch (command) {
        case 'build':
          await build(client);
          break;

        case 'test':
          const testResult = await test(client);
          if (!testResult.passed) process.exit(1);
          break;

        case 'lint':
          const lintPassed = await lint(client);
          if (!lintPassed) process.exit(1);
          break;

        case 'container':
          await buildContainers(client);
          break;

        case 'publish':
          const registry = process.env.CONTAINER_REGISTRY || 'ghcr.io/yurikrupnik';
          const tag = process.env.TAG || 'latest';
          await publishContainers(client, registry, tag);
          break;

        case 'all':
          const success = await runAll(client);
          if (!success) process.exit(1);
          break;

        default:
          console.error(`Unknown command: ${command}`);
          console.log('Usage: npm run <build|test|lint|container|publish|all>');
          process.exit(1);
      }
    },
    { LogOutput: process.stderr }
  );
}

main().catch((error) => {
  console.error('Pipeline failed:', error);
  process.exit(1);
});
