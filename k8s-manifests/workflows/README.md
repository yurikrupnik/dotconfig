# CI/CD Pipeline

Automated testing, building, deployment, and chaos testing workflows.

## Quick Start

```bash
# CI/CD runs automatically on:
# - Push to main/develop
# - Pull requests
# - Manual trigger

# View pipeline status
gh run list

# View specific run
gh run view <run-id>
```

## Pipeline Stages

1. **Test** - Unit tests, linters, security scan
2. **Build** - Multi-arch Docker image
3. **Deploy Staging** - Deploy + chaos tests
4. **Canary Production** - Flagger canary deployment
5. **Chaos Test** - Resilience validation

## Chaos Testing

Pipelines automatically run chaos experiments to validate resilience:

```bash
# Chaos experiments applied in staging
kubectl apply -f k8s-manifests/examples/chaos/pod-chaos.yaml

# Monitor resilience
kubectl logs -f deployment/dotconfig
```

## Integration

- **Flagger**: Canary deployments with traffic shifting
- **Chaos Mesh**: Resilience testing
- **KEDA**: Autoscaling validation
- **Observability**: Metrics, logs, and traces

## Documentation

See [CI-CD.md](./CI-CD.md) for complete documentation.
