# Architecture

## Overview

The platform follows a microservices architecture with Dapr as the service mesh, enabling:

- **Loose coupling** between skills and agents
- **Language agnostic** communication via HTTP/gRPC
- **Resilient** pub/sub messaging
- **Portable** across cloud providers

## Component Diagram

```
┌────────────────────────────────────────────────────────────────────────┐
│                            User / CLI                                   │
│                    (Nu Commands / Bash Scripts)                         │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────────┐
│                         Platform Orchestrator                           │
│                    (TypeScript / platform.nu)                           │
└───────────────────────────────┬────────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐       ┌───────────────┐       ┌───────────────┐
│    Skills     │       │    Agents     │       │Cloud Providers│
│  ┌─────────┐  │       │  ┌─────────┐  │       │  ┌─────────┐  │
│  │  Rust   │  │       │  │ LLM     │  │       │  │   AWS   │  │
│  │ (CPU)   │  │       │  │ Agent   │  │       │  └─────────┘  │
│  └─────────┘  │       │  └─────────┘  │       │  ┌─────────┐  │
│  ┌─────────┐  │       │       │       │       │  │   GCP   │  │
│  │   TS    │  │       │       ▼       │       │  └─────────┘  │
│  │ (LLM)   │◄─┼───────┼─► OpenRouter  │       │  ┌─────────┐  │
│  └─────────┘  │       │               │       │  │  Azure  │  │
│  ┌─────────┐  │       └───────────────┘       │  └─────────┘  │
│  │   Nu    │  │               │               └───────────────┘
│  │(Script) │  │               │                       │
│  └─────────┘  │               │                       │
└───────────────┘               │                       │
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────────┐
│                          Dapr Service Mesh                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ State Store  │  │   Pub/Sub    │  │   Service    │  │  Secrets   │  │
│  │   (Redis)    │  │   (Redis)    │  │  Invocation  │  │  (Vault)   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

## Communication Patterns

### 1. Synchronous - Service Invocation

Direct service-to-service calls via Dapr sidecar:

```typescript
// Agent invoking a skill
const result = await daprClient.invoker.invoke(
  'code-review-skill',
  'review',
  HttpMethod.POST,
  { diff: gitDiff }
);
```

### 2. Asynchronous - Pub/Sub

Event-driven communication for decoupled workflows:

```typescript
// Publish event
await daprClient.pubsub.publish('events', 'skill.completed', {
  skillId: 'code-review',
  result: reviewResult
});

// Subscribe to events
@DaprSubscribe({ pubsubName: 'events', topic: 'skill.completed' })
async handleSkillCompleted(event: SkillCompletedEvent) {
  // Process result
}
```

### 3. State Management

Persistent state across service restarts:

```typescript
// Save state
await daprClient.state.save('statestore', [
  { key: 'agent-context', value: agentState }
]);

// Get state
const state = await daprClient.state.get('statestore', 'agent-context');
```

## Skill Execution Flow

```
┌─────────┐     ┌──────────────┐     ┌─────────────┐     ┌─────────┐
│  User   │────►│ platform.nu  │────►│  Registry   │────►│  Skill  │
│         │     │              │     │ (discovery) │     │ Runtime │
└─────────┘     └──────────────┘     └─────────────┘     └────┬────┘
                                                              │
                      ┌───────────────────────────────────────┘
                      ▼
           ┌─────────────────────┐
           │   Skill Execution   │
           │  ┌───────────────┐  │
           │  │ Rust (native) │  │ ◄── CPU-intensive
           │  ├───────────────┤  │
           │  │ TS (node)     │  │ ◄── LLM-powered
           │  ├───────────────┤  │
           │  │ Nu (script)   │  │ ◄── Quick tasks
           │  └───────────────┘  │
           └─────────────────────┘
```

## Agent Lifecycle

```
┌────────────────────────────────────────────────────────────┐
│                    Agent Lifecycle                          │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌────────┐  │
│   │  Spawn  │───►│  Init   │───►│  Ready  │───►│  Stop  │  │
│   └─────────┘    └─────────┘    └────┬────┘    └────────┘  │
│                                      │                      │
│                                      ▼                      │
│                               ┌─────────────┐               │
│                               │   Invoke    │◄──┐           │
│                               │   (LLM)     │   │           │
│                               └──────┬──────┘   │           │
│                                      │          │           │
│                                      ▼          │           │
│                               ┌─────────────┐   │           │
│                               │  Execute    │───┘           │
│                               │  (Skills)   │               │
│                               └─────────────┘               │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

## State Flow

```
┌────────────────────────────────────────────────────────────┐
│                      State Storage                          │
├────────────────────────────────────────────────────────────┤
│                                                            │
│   Agent State           Skill State          Shared State  │
│   ┌─────────────┐      ┌─────────────┐      ┌───────────┐  │
│   │ • Context   │      │ • Config    │      │ • Cache   │  │
│   │ • History   │      │ • Results   │      │ • Metrics │  │
│   │ • Tools     │      │ • Metadata  │      │ • Logs    │  │
│   └──────┬──────┘      └──────┬──────┘      └─────┬─────┘  │
│          │                    │                   │         │
│          └────────────────────┼───────────────────┘         │
│                               ▼                             │
│                    ┌─────────────────────┐                  │
│                    │   Dapr State Store  │                  │
│                    │       (Redis)       │                  │
│                    └─────────────────────┘                  │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

## Deployment Options

### Local Development

```bash
# Using Docker Compose with Dapr
nu platform.nu up --dapr

# Components run as containers:
# - Redis (state + pubsub)
# - Dapr sidecar per service
# - Skills as separate processes
```

### Kubernetes

```bash
# Deploy to K8s cluster with Dapr
nu platform.nu deploy --target k8s

# Dapr injected as sidecar automatically
# Skills deployed as Deployments/Jobs
# Agents as StatefulSets (persistent identity)
```

### Cloud Native

```bash
# Deploy to cloud with managed services
nu platform.nu deploy --target gcp

# Uses:
# - Cloud Run / ECS / AKS for compute
# - Managed Redis / Memorystore for state
# - Cloud Pub/Sub for events
```
