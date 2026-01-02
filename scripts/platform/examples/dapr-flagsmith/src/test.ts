/**
 * Test script for Dapr + Flagsmith integration
 */

const BASE_URL = process.env.BASE_URL || 'http://localhost:3000';

interface TestResult {
  name: string;
  passed: boolean;
  response?: unknown;
  error?: string;
}

async function runTests(): Promise<void> {
  const results: TestResult[] = [];

  // Test 1: Health check
  results.push(await testEndpoint('Health Check', '/health'));

  // Test 2: List all features
  results.push(await testEndpoint('List Features', '/features'));

  // Test 3: Get specific feature
  results.push(await testEndpoint('Get Feature', '/features/new-dashboard'));

  // Test 4: Get feature with user targeting
  results.push(
    await testEndpoint(
      'Feature with Targeting',
      '/features/beta-features?user=premium-user'
    )
  );

  // Test 5: Evaluate feature with context (POST)
  results.push(
    await testEndpoint('Evaluate Feature', '/features/dark-mode/evaluate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        context: {
          targetingKey: 'user-123',
          plan: 'enterprise',
        },
        defaultValue: false,
        valueType: 'boolean',
      }),
    })
  );

  // Test 6: Demo endpoint
  results.push(await testEndpoint('Demo', '/demo?user=test-user'));

  // Print results
  console.log('\n=== Test Results ===\n');

  let passed = 0;
  let failed = 0;

  for (const result of results) {
    const icon = result.passed ? '✅' : '❌';
    console.log(`${icon} ${result.name}`);

    if (!result.passed && result.error) {
      console.log(`   Error: ${result.error}`);
    }

    if (result.passed) {
      passed++;
    } else {
      failed++;
    }
  }

  console.log(`\nTotal: ${passed} passed, ${failed} failed\n`);

  process.exit(failed > 0 ? 1 : 0);
}

async function testEndpoint(
  name: string,
  path: string,
  options?: RequestInit
): Promise<TestResult> {
  try {
    const response = await fetch(`${BASE_URL}${path}`, options);
    const data = await response.json();

    if (!response.ok) {
      return {
        name,
        passed: false,
        error: `HTTP ${response.status}: ${JSON.stringify(data)}`,
      };
    }

    console.log(`\n[${name}] Response:`, JSON.stringify(data, null, 2));

    return {
      name,
      passed: true,
      response: data,
    };
  } catch (error) {
    return {
      name,
      passed: false,
      error: String(error),
    };
  }
}

runTests().catch(console.error);
