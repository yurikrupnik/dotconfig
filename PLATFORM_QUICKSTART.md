# Platform Quick Start

Get started with the enterprise cloud-native platform in 5 minutes.

## Local Development (Docker Compose)

```bash
# Start all services locally
docker compose up -d

# Access services
# Application: http://localhost:8080
# Flagsmith: http://localhost:3000
# Grafana: http://localhost:3001 (admin/admin)
# Prometheus: http://localhost:9090
# Jaeger: http://localhost:16686

# View logs
docker compose logs -f dotconfig-app

# Stop services
docker compose down
```

## Kubernetes Deployment

```bash
# Install entire platform stack
nu scripts/nu/platform/stack.nu install-all

# Deploy example application with all integrations
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/
kubectl apply -f k8s-manifests/examples/canary-deployment/
kubectl apply -f k8s-manifests/examples/chaos/

# Check status
kubectl get pods,canaries,scaledobjects
```

## Key Features

### Feature Flags (Flagsmith)
```bash
# Access UI
open http://localhost:3000

# Create flags and toggle them in real-time
```

### Progressive Delivery (Flagger)
```bash
# Trigger canary deployment
kubectl set image deployment/dotconfig dotconfig=v2.0

# Monitor progress
kubectl get canary dotconfig -w
```

### Autoscaling (KEDA)
```bash
# Generate load to trigger scaling
hey -n 10000 -c 100 http://dotconfig/

# Watch pods scale
watch kubectl get pods -l app=dotconfig
```

### Chaos Engineering
```bash
# Apply chaos experiments
kubectl apply -f k8s-manifests/examples/chaos/experiments.yaml

# Monitor resilience
kubectl logs -f deployment/dotconfig
```

### Cloud Resources (Crossplane)
```bash
# Deploy AWS resources via Kubernetes
kubectl apply -f k8s-manifests/examples/crossplane/resources.yaml

# Monitor creation
kubectl get managed
```

## Documentation

- [Full Platform Documentation](PLATFORM.md)
- [Local Development Guide](LOCAL_DEV.md)
- [Example Manifests](k8s-manifests/examples/README.md)
