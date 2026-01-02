# Dapr + Flagsmith Feature Flags Example

This example demonstrates how to integrate Dapr with Flagsmith for feature flag management using OpenFeature.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Application                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Express   │  │ OpenFeature │  │    Dapr Client      │  │
│  │   Server    │──│    SDK      │──│  (State, PubSub)    │  │
│  └─────────────┘  └──────┬──────┘  └──────────┬──────────┘  │
│                          │                     │             │
└──────────────────────────┼─────────────────────┼─────────────┘
                           │                     │
                           ▼                     ▼
                    ┌─────────────┐       ┌─────────────┐
                    │  Flagsmith  │       │Dapr Sidecar │
                    │   Server    │       │  (Redis)    │
                    └─────────────┘       └─────────────┘
```

## Features

- **OpenFeature SDK**: Vendor-agnostic feature flag API
- **Flagsmith Provider**: Connect to Flagsmith for flag management
- **Dapr State Store**: Cache flag values locally
- **Dapr Pub/Sub**: Broadcast flag changes to all instances
- **Targeting**: User-based feature targeting

## Quick Start

### 1. Start Flagsmith (Docker)

```bash
docker run -d --name flagsmith \
  -p 8000:8000 \
  -e DJANGO_ALLOWED_HOSTS="*" \
  flagsmith/flagsmith:latest
```

### 2. Configure Flagsmith

1. Open http://localhost:8000
2. Create an account
3. Create a project
4. Create flags: `new-dashboard`, `beta-features`, `dark-mode`
5. Copy the Environment Key

### 3. Set Environment Variables

```bash
export FLAGSMITH_API_KEY=your-environment-key
export FLAGSMITH_API_URL=http://localhost:8000/api/v1/
```

### 4. Start with Dapr

```bash
npm install
npm run dapr
```

### 5. Test Endpoints

```bash
# Check feature flags
curl http://localhost:3000/features

# Check specific flag
curl http://localhost:3000/features/new-dashboard

# Check with user targeting
curl http://localhost:3000/features/beta-features?user=premium-user

# Toggle flag (via Flagsmith UI or API)
# Changes propagate via Dapr pub/sub
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/features` | GET | List all feature flags |
| `/features/:name` | GET | Get specific flag value |
| `/features/:name/evaluate` | POST | Evaluate with context |

## Configuration

### Dapr Components

**components/statestore.yaml** - Cache flags locally
**components/pubsub.yaml** - Broadcast flag changes

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FLAGSMITH_API_KEY` | Flagsmith environment key | Required |
| `FLAGSMITH_API_URL` | Flagsmith API URL | https://edge.api.flagsmith.com/api/v1/ |
| `CACHE_TTL_SECONDS` | Flag cache TTL | 60 |
| `PORT` | Server port | 3000 |

## Code Examples

### Basic Flag Check

```typescript
const client = OpenFeature.getClient();
const isEnabled = await client.getBooleanValue('new-dashboard', false);
```

### With User Targeting

```typescript
const context = { targetingKey: 'user-123', plan: 'premium' };
const isEnabled = await client.getBooleanValue('beta-features', false, context);
```

### Listening to Flag Changes

```typescript
client.addHandler(ProviderEvents.ConfigurationChanged, (event) => {
  console.log('Flags changed:', event.flagsChanged);
});
```
