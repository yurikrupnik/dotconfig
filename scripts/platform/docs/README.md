# Skills & Agents Management Platform

A hybrid management platform for LLM-powered agents and CLI tools.

## Quick Start

```bash
# Initialize the platform
nu scripts/platform/commands/platform.nu init

# Start the platform with Dapr
nu scripts/platform/commands/platform.nu up --dapr

# List available skills
nu scripts/platform/commands/skill.nu list

# Spawn an LLM agent
nu scripts/platform/commands/agent.nu spawn my-agent --model openai/gpt-4
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Platform                                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   Skills    │  │   Agents    │  │    Cloud Providers      │  │
│  │  (Rust/TS)  │  │  (LLM/Dapr) │  │   (AWS/GCP/Azure)       │  │
│  └──────┬──────┘  └──────┬──────┘  └────────────┬────────────┘  │
│         │                │                       │               │
│  ┌──────┴────────────────┴───────────────────────┴────────────┐ │
│  │                    Dapr Service Mesh                        │ │
│  │  • State Store  • Pub/Sub  • Service Invocation  • Secrets │ │
│  └──────────────────────────┬─────────────────────────────────┘ │
│                             │                                    │
│  ┌──────────────────────────┴─────────────────────────────────┐ │
│  │                      OpenRouter                             │ │
│  │  • Model Routing  • Fallbacks  • Rate Limiting              │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Components

| Component | Technology | Purpose |
|-----------|------------|---------|
| Skills | Rust, TypeScript, Nushell | Reusable capabilities |
| Agents | TypeScript + Dapr | LLM-powered autonomous actors |
| Cloud Providers | TypeScript | Unified multi-cloud API |
| Service Mesh | Dapr | Communication, state, pub/sub |
| LLM Routing | OpenRouter | Model selection and fallback |

## Directory Structure

```
scripts/platform/
├── docs/               # Documentation
├── skills/             # Skill registry and templates
│   ├── registry.json   # Skill discovery
│   ├── templates/      # Skill scaffolding
│   └── examples/       # Example implementations
├── agents/             # Agent registry and configs
│   ├── registry.json   # Agent discovery
│   ├── config/         # Dapr & OpenRouter configs
│   └── templates/      # Agent scaffolding
├── cloud/              # Cloud provider wrapper (TS)
└── commands/           # Nu management commands
```

## Technology Choices

- **Rust**: CPU-intensive skills (file processing, code analysis)
- **TypeScript**: Platform management, cloud wrappers, LLM orchestration
- **Nushell**: CLI commands, scripting, quick skills
- **Dapr**: Agent-to-agent communication, state, pub/sub, secrets
- **OpenRouter**: LLM routing with automatic fallback

## Documentation

- [Architecture](./architecture.md) - System design and patterns
- [Skills](./skills.md) - Skill development guide
- [Agents](./agents.md) - Agent development guide
- [Dapr Integration](./dapr-integration.md) - Service mesh setup
- [OpenRouter](./openrouter.md) - LLM routing configuration
- [Cloud Providers](./cloud-providers.md) - Multi-cloud API reference
