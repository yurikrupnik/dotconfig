/**
 * Dapr + Flagsmith Feature Flags Server
 *
 * Integrates:
 * - OpenFeature SDK for vendor-agnostic feature flags
 * - Flagsmith provider for flag management
 * - Dapr for state caching and pub/sub
 */

import express, { Request, Response } from 'express';
import { DaprClient } from '@dapr/dapr';
import { OpenFeature, Client, EvaluationContext, ProviderEvents } from '@openfeature/server-sdk';
import { FlagsmithProvider } from './flagsmith-provider.js';

// Configuration
const PORT = parseInt(process.env.PORT || '3000');
const FLAGSMITH_API_KEY = process.env.FLAGSMITH_API_KEY || '';
const FLAGSMITH_API_URL = process.env.FLAGSMITH_API_URL || 'https://edge.api.flagsmith.com/api/v1/';
const CACHE_TTL_SECONDS = parseInt(process.env.CACHE_TTL_SECONDS || '60');

// Initialize Dapr client
const dapr = new DaprClient();

// Initialize Express
const app = express();
app.use(express.json());

// OpenFeature client
let featureClient: Client;

/**
 * Initialize OpenFeature with Flagsmith provider
 */
async function initializeFeatureFlags(): Promise<void> {
  console.log('Initializing feature flags...');

  // Create Flagsmith provider with Dapr integration
  const provider = new FlagsmithProvider({
    apiKey: FLAGSMITH_API_KEY,
    apiUrl: FLAGSMITH_API_URL,
    cacheTtlSeconds: CACHE_TTL_SECONDS,
    daprClient: dapr,
    stateStoreName: 'statestore',
    pubsubName: 'events',
    pubsubTopic: 'feature-flags.changed',
  });

  // Set the provider
  await OpenFeature.setProviderAndWait(provider);

  // Get client
  featureClient = OpenFeature.getClient();

  // Listen for configuration changes
  featureClient.addHandler(ProviderEvents.ConfigurationChanged, (event) => {
    console.log('Feature flags changed:', event);
  });

  console.log('Feature flags initialized');
}

/**
 * Health check endpoint
 */
app.get('/health', (_, res: Response) => {
  res.json({ status: 'healthy', service: 'feature-flags' });
});

/**
 * List all feature flags
 */
app.get('/features', async (_, res: Response) => {
  try {
    // Get all flags from provider
    const provider = OpenFeature.getProvider() as FlagsmithProvider;
    const flags = await provider.getAllFlags();

    res.json({
      flags,
      cached: provider.isCached(),
      lastUpdated: provider.getLastUpdated(),
    });
  } catch (error) {
    res.status(500).json({ error: String(error) });
  }
});

/**
 * Get specific feature flag value
 */
app.get('/features/:name', async (req: Request, res: Response) => {
  const { name } = req.params;
  const { user, ...attributes } = req.query;

  try {
    // Build evaluation context
    const context: EvaluationContext = {};
    if (user) {
      context.targetingKey = String(user);
    }
    // Add custom attributes
    Object.entries(attributes).forEach(([key, value]) => {
      context[key] = String(value);
    });

    // Evaluate flag
    const evaluation = await featureClient.getBooleanDetails(name, false, context);

    res.json({
      flag: name,
      value: evaluation.value,
      variant: evaluation.variant,
      reason: evaluation.reason,
      context,
    });
  } catch (error) {
    res.status(500).json({ error: String(error) });
  }
});

/**
 * Evaluate feature flag with full context (POST)
 */
app.post('/features/:name/evaluate', async (req: Request, res: Response) => {
  const { name } = req.params;
  const { context, defaultValue = false, valueType = 'boolean' } = req.body;

  try {
    let evaluation;

    switch (valueType) {
      case 'boolean':
        evaluation = await featureClient.getBooleanDetails(name, defaultValue, context);
        break;
      case 'string':
        evaluation = await featureClient.getStringDetails(name, defaultValue, context);
        break;
      case 'number':
        evaluation = await featureClient.getNumberDetails(name, defaultValue, context);
        break;
      case 'object':
        evaluation = await featureClient.getObjectDetails(name, defaultValue, context);
        break;
      default:
        throw new Error(`Unknown value type: ${valueType}`);
    }

    res.json({
      flag: name,
      value: evaluation.value,
      variant: evaluation.variant,
      reason: evaluation.reason,
      flagMetadata: evaluation.flagMetadata,
    });
  } catch (error) {
    res.status(500).json({ error: String(error) });
  }
});

/**
 * Webhook endpoint for Flagsmith changes
 */
app.post('/webhooks/flagsmith', async (req: Request, res: Response) => {
  console.log('Received Flagsmith webhook:', req.body);

  try {
    // Invalidate cache
    const provider = OpenFeature.getProvider() as FlagsmithProvider;
    await provider.refreshFlags();

    // Publish change event via Dapr
    await dapr.pubsub.publish('events', 'feature-flags.changed', {
      timestamp: new Date().toISOString(),
      source: 'webhook',
      changes: req.body,
    });

    res.json({ status: 'processed' });
  } catch (error) {
    res.status(500).json({ error: String(error) });
  }
});

/**
 * Subscribe to flag change events (Dapr)
 */
app.post('/dapr/subscribe', async (_, res: Response) => {
  res.json([
    {
      pubsubname: 'events',
      topic: 'feature-flags.changed',
      route: '/events/flags-changed',
    },
  ]);
});

/**
 * Handle flag change events
 */
app.post('/events/flags-changed', async (req: Request, res: Response) => {
  console.log('Flag change event received:', req.body);

  // Refresh local cache
  const provider = OpenFeature.getProvider() as FlagsmithProvider;
  await provider.refreshFlags();

  res.json({ status: 'ok' });
});

/**
 * Demo endpoint showing feature flags in action
 */
app.get('/demo', async (req: Request, res: Response) => {
  const user = String(req.query.user || 'anonymous');

  const context: EvaluationContext = {
    targetingKey: user,
  };

  // Check various flags
  const [newDashboard, betaFeatures, darkMode] = await Promise.all([
    featureClient.getBooleanValue('new-dashboard', false, context),
    featureClient.getBooleanValue('beta-features', false, context),
    featureClient.getBooleanValue('dark-mode', false, context),
  ]);

  res.json({
    user,
    features: {
      newDashboard,
      betaFeatures,
      darkMode,
    },
    message: newDashboard
      ? 'Welcome to the new dashboard!'
      : 'Using classic dashboard',
  });
});

// Start server
async function main(): Promise<void> {
  await initializeFeatureFlags();

  app.listen(PORT, () => {
    console.log(`Feature flags server running on port ${PORT}`);
    console.log(`Flagsmith API: ${FLAGSMITH_API_URL}`);
  });
}

main().catch(console.error);
