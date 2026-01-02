# Rust Example Applications

Production-ready Rust examples demonstrating platform integration patterns.

## Applications

### 1. Queue Worker (`queue-worker`)

KEDA-scalable Redis queue worker for background task processing.

**Features:**
- Graceful shutdown on SIGTERM
- Configurable concurrency
- Task retry with exponential backoff
- Prometheus metrics endpoint
- Dead letter queue support

**Usage:**
```bash
# Run locally
REDIS_URL=redis://localhost:6379 cargo run -p queue-worker

# With Docker
docker run -e REDIS_URL=redis://redis:6379 queue-worker
```

**Environment Variables:**
| Variable | Default | Description |
|----------|---------|-------------|
| `REDIS_URL` | `redis://localhost:6379` | Redis connection URL |
| `QUEUE_NAME` | `tasks` | Queue to process |
| `CONCURRENCY` | `10` | Max concurrent tasks |
| `HEALTH_PORT` | `8080` | Health check port |
| `VISIBILITY_TIMEOUT` | `30` | Task visibility timeout (seconds) |

---

### 2. Dapr Service (`dapr-service`)

Microservice demonstrating Dapr integration patterns.

**Features:**
- State management (Redis)
- Pub/Sub messaging
- Service-to-service invocation
- Secrets management
- RESTful API

**Endpoints:**
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET/POST/DELETE | `/state/:key` | State operations |
| POST | `/tasks` | Create task |
| GET | `/tasks/:id` | Get task |
| POST | `/tasks/:id/complete` | Complete task |
| POST | `/invoke/:service/:method` | Invoke other service |

**Usage:**
```bash
# Run with Dapr
dapr run --app-id dapr-service --app-port 3000 -- cargo run -p dapr-service

# Or with Docker Compose (see docker-compose.yaml)
```

---

### 3. LLM Agent (`llm-agent`)

AI-powered agent using OpenRouter with tool calling.

**Features:**
- OpenRouter integration with model fallback
- Tool/function calling (read_file, write_file, execute_command, search_code)
- Conversation memory
- Rate limiting handling

**Endpoints:**
| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/invoke` | Invoke agent with message |
| GET | `/info` | Agent information |

**Usage:**
```bash
# Run locally
OPENROUTER_API_KEY=your-key cargo run -p llm-agent

# Invoke
curl -X POST http://localhost:3001/invoke \
  -H "Content-Type: application/json" \
  -d '{"message": "Help me refactor this function"}'
```

**Environment Variables:**
| Variable | Default | Description |
|----------|---------|-------------|
| `OPENROUTER_API_KEY` | (required) | OpenRouter API key |
| `DEFAULT_MODEL` | `anthropic/claude-3.5-sonnet` | Primary model |
| `FALLBACK_MODEL` | `openai/gpt-4o` | Fallback model |
| `MAX_TOKENS` | `4096` | Max response tokens |
| `PORT` | `3001` | Service port |

---

### 4. File Processor (`file-processor`)

CPU-intensive file processing skill using Rayon for parallelism.

**Features:**
- Parallel file scanning
- SHA-256 hashing with memory-mapped files
- Duplicate file detection
- Code analysis (lines, complexity, issues)

**Operations:**
| Operation | Description |
|-----------|-------------|
| `scan` | Scan directory for files |
| `hash` | Calculate SHA-256 hash |
| `duplicates` | Find duplicate files |
| `analyze` | Analyze code files |

**Endpoints:**
| Method | Path | Description |
|--------|------|-------------|
| POST | `/execute` | Execute skill with SkillInput |
| POST | `/scan` | Scan directory |
| POST | `/hash` | Hash file |
| POST | `/duplicates` | Find duplicates |
| POST | `/analyze` | Analyze code |

**Usage:**
```bash
# Run locally
cargo run -p file-processor

# Analyze code
curl -X POST http://localhost:3002/analyze \
  -H "Content-Type: application/json" \
  -d '{"path": "/path/to/project"}'
```

---

## Building

### Local Development

```bash
# Build all
cargo build --workspace

# Build specific app
cargo build -p queue-worker

# Run tests
cargo test --workspace

# Run with release optimizations
cargo build --release --workspace
```

### Docker

```bash
# Build all images
docker compose build

# Build specific image
docker build -f Dockerfile --target queue-worker -t queue-worker .
```

---

## Deployment

### Kubernetes with KEDA

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: queue-worker-scaler
spec:
  scaleTargetRef:
    name: queue-worker
  pollingInterval: 15
  cooldownPeriod: 300
  minReplicaCount: 0
  maxReplicaCount: 20
  triggers:
    - type: redis
      metadata:
        address: redis:6379
        listName: tasks
        listLength: "5"
```

### Dapr Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: dapr-service
spec:
  template:
    metadata:
      annotations:
        dapr.io/enabled: "true"
        dapr.io/app-id: "dapr-service"
        dapr.io/app-port: "3000"
```

---

## Shared Library

The `shared` crate provides common types and traits:

- `Skill` trait for CPU-intensive workloads
- `Agent` trait for LLM-powered agents
- `Task` and `TaskResult` for queue processing
- `SkillInput` and `SkillOutput` for skill execution
- `AgentMessage` and `AgentResponse` for agent communication

```rust
use shared::{Skill, SkillInput, SkillOutput, SkillError};

#[async_trait]
impl Skill for MySkill {
    fn name(&self) -> &str { "my-skill" }
    fn description(&self) -> &str { "My custom skill" }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput, SkillError> {
        // Implementation
    }
}
```

---

## Architecture

```
┌─────────────────┐     ┌─────────────────┐
│   LLM Agent     │────▶│   OpenRouter    │
│   (llm-agent)   │     │   (Claude/GPT)  │
└─────────────────┘     └─────────────────┘
         │
         ▼ Dapr Service Invocation
┌─────────────────┐     ┌─────────────────┐
│  Dapr Service   │────▶│     Redis       │
│ (dapr-service)  │     │  (State/PubSub) │
└─────────────────┘     └─────────────────┘
         │
         ▼ Pub/Sub
┌─────────────────┐     ┌─────────────────┐
│  Queue Worker   │◀────│     Redis       │
│ (queue-worker)  │     │    (Queue)      │
└─────────────────┘     └─────────────────┘
         │
         ▼ Task Execution
┌─────────────────┐
│ File Processor  │
│(file-processor) │
└─────────────────┘
```
