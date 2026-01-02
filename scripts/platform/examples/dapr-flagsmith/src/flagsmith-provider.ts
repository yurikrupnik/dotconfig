/**
 * Flagsmith OpenFeature Provider with Dapr Integration
 *
 * Features:
 * - OpenFeature provider interface
 * - Dapr state store for caching
 * - Dapr pub/sub for change notifications
 * - Automatic cache refresh
 */

import {
  Provider,
  ProviderMetadata,
  ResolutionDetails,
  EvaluationContext,
  JsonValue,
  ProviderStatus,
  OpenFeatureEventEmitter,
  ProviderEvents,
} from '@openfeature/server-sdk';
import { DaprClient } from '@dapr/dapr';

interface FlagsmithFlag {
  id: number;
  feature: {
    id: number;
    name: string;
    type: string;
  };
  enabled: boolean;
  value?: string | number | boolean | object;
}

interface FlagsmithIdentity {
  identifier: string;
  traits?: Record<string, string | number | boolean>;
}

interface FlagsmithProviderConfig {
  apiKey: string;
  apiUrl?: string;
  cacheTtlSeconds?: number;
  daprClient?: DaprClient;
  stateStoreName?: string;
  pubsubName?: string;
  pubsubTopic?: string;
}

export class FlagsmithProvider implements Provider {
  readonly metadata: ProviderMetadata = {
    name: 'flagsmith-dapr',
  };

  readonly rulesGlobally = false;
  private config: Required<FlagsmithProviderConfig>;
  private dapr?: DaprClient;
  private flags: Map<string, FlagsmithFlag> = new Map();
  private lastUpdated: Date | null = null;
  private cacheValid = false;
  private status: ProviderStatus = ProviderStatus.NOT_READY;
  events = new OpenFeatureEventEmitter();

  constructor(config: FlagsmithProviderConfig) {
    this.config = {
      apiKey: config.apiKey,
      apiUrl: config.apiUrl || 'https://edge.api.flagsmith.com/api/v1/',
      cacheTtlSeconds: config.cacheTtlSeconds || 60,
      daprClient: config.daprClient!,
      stateStoreName: config.stateStoreName || 'statestore',
      pubsubName: config.pubsubName || 'events',
      pubsubTopic: config.pubsubTopic || 'feature-flags.changed',
    };
    this.dapr = config.daprClient;
  }

  /**
   * Initialize the provider
   */
  async initialize(): Promise<void> {
    console.log('Initializing Flagsmith provider...');

    // Try to load from Dapr cache first
    if (this.dapr) {
      try {
        const cached = await this.dapr.state.get(
          this.config.stateStoreName,
          'flagsmith-flags'
        );
        if (cached) {
          const data = cached as { flags: FlagsmithFlag[]; timestamp: string };
          const age = Date.now() - new Date(data.timestamp).getTime();

          if (age < this.config.cacheTtlSeconds * 1000) {
            this.loadFlagsFromArray(data.flags);
            this.cacheValid = true;
            console.log(`Loaded ${this.flags.size} flags from Dapr cache`);
          }
        }
      } catch (e) {
        console.warn('Failed to load from Dapr cache:', e);
      }
    }

    // Fetch from Flagsmith if cache miss
    if (!this.cacheValid) {
      await this.refreshFlags();
    }

    this.status = ProviderStatus.READY;
    console.log('Flagsmith provider initialized');
  }

  /**
   * Shutdown the provider
   */
  async onClose(): Promise<void> {
    this.status = ProviderStatus.NOT_READY;
  }

  /**
   * Refresh flags from Flagsmith API
   */
  async refreshFlags(): Promise<void> {
    try {
      const response = await fetch(`${this.config.apiUrl}flags/`, {
        headers: {
          'X-Environment-Key': this.config.apiKey,
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        throw new Error(`Flagsmith API error: ${response.status}`);
      }

      const flags: FlagsmithFlag[] = await response.json();
      this.loadFlagsFromArray(flags);
      this.lastUpdated = new Date();
      this.cacheValid = true;

      // Save to Dapr cache
      if (this.dapr) {
        await this.dapr.state.save(this.config.stateStoreName, [
          {
            key: 'flagsmith-flags',
            value: { flags, timestamp: this.lastUpdated.toISOString() },
          },
        ]);
      }

      console.log(`Refreshed ${this.flags.size} flags from Flagsmith`);

      // Emit configuration changed event
      this.events.emit(ProviderEvents.ConfigurationChanged, {
        flagsChanged: Array.from(this.flags.keys()),
      });
    } catch (error) {
      console.error('Failed to refresh flags:', error);
      throw error;
    }
  }

  private loadFlagsFromArray(flags: FlagsmithFlag[]): void {
    this.flags.clear();
    for (const flag of flags) {
      this.flags.set(flag.feature.name, flag);
    }
  }

  /**
   * Get all flags
   */
  async getAllFlags(): Promise<Record<string, { enabled: boolean; value?: JsonValue }>> {
    if (!this.cacheValid) {
      await this.refreshFlags();
    }

    const result: Record<string, { enabled: boolean; value?: JsonValue }> = {};
    for (const [name, flag] of this.flags) {
      result[name] = {
        enabled: flag.enabled,
        value: flag.value as JsonValue,
      };
    }
    return result;
  }

  /**
   * Check if using cached data
   */
  isCached(): boolean {
    return this.cacheValid;
  }

  /**
   * Get last update time
   */
  getLastUpdated(): Date | null {
    return this.lastUpdated;
  }

  /**
   * Resolve boolean flag
   */
  async resolveBooleanEvaluation(
    flagKey: string,
    defaultValue: boolean,
    context: EvaluationContext
  ): Promise<ResolutionDetails<boolean>> {
    return this.resolveFlag(flagKey, defaultValue, context, 'boolean');
  }

  /**
   * Resolve string flag
   */
  async resolveStringEvaluation(
    flagKey: string,
    defaultValue: string,
    context: EvaluationContext
  ): Promise<ResolutionDetails<string>> {
    return this.resolveFlag(flagKey, defaultValue, context, 'string');
  }

  /**
   * Resolve number flag
   */
  async resolveNumberEvaluation(
    flagKey: string,
    defaultValue: number,
    context: EvaluationContext
  ): Promise<ResolutionDetails<number>> {
    return this.resolveFlag(flagKey, defaultValue, context, 'number');
  }

  /**
   * Resolve object flag
   */
  async resolveObjectEvaluation<T extends JsonValue>(
    flagKey: string,
    defaultValue: T,
    context: EvaluationContext
  ): Promise<ResolutionDetails<T>> {
    return this.resolveFlag(flagKey, defaultValue, context, 'object');
  }

  /**
   * Generic flag resolution
   */
  private async resolveFlag<T>(
    flagKey: string,
    defaultValue: T,
    context: EvaluationContext,
    valueType: string
  ): Promise<ResolutionDetails<T>> {
    // Check cache validity
    if (!this.cacheValid) {
      await this.refreshFlags();
    }

    const flag = this.flags.get(flagKey);

    if (!flag) {
      return {
        value: defaultValue,
        reason: 'DEFAULT',
        errorCode: 'FLAG_NOT_FOUND',
      };
    }

    // If targeting key present, fetch identity-specific flags
    if (context.targetingKey) {
      try {
        const identityFlags = await this.getIdentityFlags(context);
        const identityFlag = identityFlags.get(flagKey);
        if (identityFlag) {
          return {
            value: this.coerceValue(identityFlag, valueType) as T,
            reason: 'TARGETING_MATCH',
            variant: identityFlag.enabled ? 'on' : 'off',
          };
        }
      } catch (e) {
        console.warn('Failed to fetch identity flags:', e);
      }
    }

    // Return cached flag value
    if (!flag.enabled) {
      return {
        value: defaultValue,
        reason: 'DISABLED',
        variant: 'off',
      };
    }

    return {
      value: this.coerceValue(flag, valueType) as T,
      reason: 'STATIC',
      variant: 'on',
    };
  }

  /**
   * Get flags for specific identity
   */
  private async getIdentityFlags(
    context: EvaluationContext
  ): Promise<Map<string, FlagsmithFlag>> {
    const identity: FlagsmithIdentity = {
      identifier: context.targetingKey!,
      traits: {},
    };

    // Add traits from context
    for (const [key, value] of Object.entries(context)) {
      if (key !== 'targetingKey' && typeof value !== 'object') {
        identity.traits![key] = value as string | number | boolean;
      }
    }

    const response = await fetch(`${this.config.apiUrl}identities/`, {
      method: 'POST',
      headers: {
        'X-Environment-Key': this.config.apiKey,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(identity),
    });

    if (!response.ok) {
      throw new Error(`Flagsmith identity API error: ${response.status}`);
    }

    const data = await response.json();
    const flags = new Map<string, FlagsmithFlag>();

    for (const flag of data.flags || []) {
      flags.set(flag.feature.name, flag);
    }

    return flags;
  }

  /**
   * Coerce flag value to expected type
   */
  private coerceValue(flag: FlagsmithFlag, valueType: string): unknown {
    if (valueType === 'boolean') {
      return flag.enabled;
    }

    const value = flag.value;

    switch (valueType) {
      case 'string':
        return String(value ?? '');
      case 'number':
        return Number(value ?? 0);
      case 'object':
        if (typeof value === 'string') {
          try {
            return JSON.parse(value);
          } catch {
            return {};
          }
        }
        return value ?? {};
      default:
        return value;
    }
  }
}
