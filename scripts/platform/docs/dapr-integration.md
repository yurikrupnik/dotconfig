# Dapr Integration Guide

## Overview

Dapr (Distributed Application Runtime) provides the service mesh for the platform:

- **State Store**: Persistent key-value storage
- **Pub/Sub**: Event-driven messaging
- **Service Invocation**: Service-to-service calls
- **Secrets**: Secure secret management
- **Actors**: Virtual actor pattern (optional)

## Installation

### Local Development

```bash
# Install Dapr CLI
brew install dapr/tap/dapr-cli

# Initialize Dapr (installs Redis, Zipkin)
dapr init

# Verify installation
dapr --version
```

### Kubernetes

```bash
# Install Dapr on K8s
dapr init -k

# Or with Helm
helm repo add dapr https://dapr.github.io/helm-charts/
helm install dapr dapr/dapr --namespace dapr-system --create-namespace
```

## Component Configuration

### State Store (Redis)

**agents/config/dapr/statestore.yaml:**
```yaml
apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: statestore
spec:
  type: state.redis
  version: v1
  metadata:
    - name: redisHost
      value: localhost:6379
    - name: redisPassword
      value: ""
    - name: actorStateStore
      value: "true"
  scopes:
    - platform
    - agents
    - skills
```

### Pub/Sub (Redis)

**agents/config/dapr/pubsub.yaml:**
```yaml
apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: events
spec:
  type: pubsub.redis
  version: v1
  metadata:
    - name: redisHost
      value: localhost:6379
    - name: redisPassword
      value: ""
    - name: consumerID
      value: "platform"
  scopes:
    - platform
    - agents
    - skills
```

### Secret Store

**agents/config/dapr/secretstore.yaml:**
```yaml
apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: secretstore
spec:
  type: secretstores.local.file
  version: v1
  metadata:
    - name: secretsFile
      value: ./secrets.json
    - name: nestedSeparator
      value: ":"
---
# For production: use cloud secret stores
# GCP Secret Manager
apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: gcp-secrets
spec:
  type: secretstores.gcp.secretmanager
  version: v1
  metadata:
    - name: project_id
      value: "my-project"
    - name: type
      value: "service_account"
```

## Usage Patterns

### State Management

```typescript
import { DaprClient } from '@dapr/dapr';

const dapr = new DaprClient();

// Save state
await dapr.state.save('statestore', [
  { key: 'user:123', value: { name: 'Alice', role: 'admin' } },
  { key: 'config', value: { maxRetries: 3 } },
]);

// Get state
const user = await dapr.state.get('statestore', 'user:123');

// Delete state
await dapr.state.delete('statestore', 'user:123');

// Bulk get
const states = await dapr.state.getBulk('statestore', ['user:123', 'config']);

// Transaction (atomic)
await dapr.state.transaction('statestore', {
  operations: [
    { operation: 'upsert', request: { key: 'balance', value: 100 } },
    { operation: 'delete', request: { key: 'temp' } },
  ],
});
```

### Pub/Sub Messaging

**Publishing:**
```typescript
// Publish event
await dapr.pubsub.publish('events', 'skill.completed', {
  skillId: 'code-review',
  status: 'success',
  result: { issues: 3 },
  timestamp: new Date().toISOString(),
});

// Publish with metadata
await dapr.pubsub.publish('events', 'agent.action', {
  agentId: 'assistant',
  action: 'invoke-skill',
}, {
  metadata: { ttlInSeconds: '300' },
});
```

**Subscribing (Express):**
```typescript
import express from 'express';
import { DaprServer } from '@dapr/dapr';

const app = express();
const daprServer = new DaprServer();

// Subscribe to topic
daprServer.pubsub.subscribe('events', 'skill.completed', async (data) => {
  console.log('Skill completed:', data);
  // Process event
});

// Start server
await daprServer.start();
```

### Service Invocation

```typescript
// Invoke another service
const result = await dapr.invoker.invoke(
  'code-review-skill',  // app-id
  'review',             // method
  {
    method: 'POST',
    body: { diff: gitDiff },
    headers: { 'Content-Type': 'application/json' },
  }
);

// GET request
const status = await dapr.invoker.invoke(
  'platform',
  'health',
  { method: 'GET' }
);
```

### Secrets

```typescript
// Get secret
const secret = await dapr.secret.get('secretstore', 'openrouter-api-key');
const apiKey = secret['openrouter-api-key'];

// Get bulk secrets
const secrets = await dapr.secret.getBulk('secretstore');
```

## Running with Dapr

### Local Development

```bash
# Run a skill with Dapr sidecar
dapr run --app-id code-review-skill \
         --app-port 3000 \
         --dapr-http-port 3500 \
         --components-path ./agents/config/dapr \
         -- npm start

# Run multiple services
dapr run --app-id platform --app-port 3001 -- nu platform.nu serve &
dapr run --app-id agent --app-port 3002 -- npm run agent &
```

### Docker Compose

```yaml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  platform:
    build: .
    environment:
      - DAPR_HTTP_PORT=3500
    depends_on:
      - redis

  platform-dapr:
    image: daprio/daprd:latest
    command: [
      "./daprd",
      "--app-id", "platform",
      "--app-port", "3000",
      "--dapr-http-port", "3500",
      "--components-path", "/components"
    ]
    volumes:
      - ./agents/config/dapr:/components
    network_mode: "service:platform"
```

## Event Topics

Standard event topics for the platform:

| Topic | Publisher | Description |
|-------|-----------|-------------|
| `skill.registered` | platform | New skill added |
| `skill.completed` | skills | Skill execution done |
| `skill.failed` | skills | Skill execution error |
| `agent.spawned` | platform | Agent started |
| `agent.stopped` | agents | Agent shutdown |
| `agent.message` | agents | Agent-to-agent message |
| `agent.action` | agents | Agent performed action |

## Debugging

```bash
# View Dapr logs
dapr logs --app-id platform

# Dashboard (local)
dapr dashboard

# Check component status
dapr components -k  # Kubernetes
dapr components     # Local
```

## Best Practices

1. **Scope components**: Limit which apps can access each component
2. **Use TTLs**: Set expiration on state and messages
3. **Handle failures**: Implement retry with exponential backoff
4. **Monitor events**: Use tracing (Zipkin/Jaeger) for visibility
5. **Secure secrets**: Use cloud secret stores in production
