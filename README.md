# .config

Enterprise cloud-native platform for dotconfig with comprehensive observability, feature flags, chaos engineering, and progressive delivery.

## 🚀 Quick Start

### Local Development

```bash
# Start all services locally
docker compose up -d

# Access services
# Application: http://localhost:8080
# Flagsmith UI: http://localhost:3000
# Grafana: http://localhost:3001 (admin/admin)
# Prometheus: http://localhost:9090
# Jaeger: http://localhost:16686

# View logs
docker compose logs -f dotconfig-app
```

### Kubernetes Deployment

```bash
# Install entire platform stack
nu scripts/nu/platform/stack.nu install-all

# Deploy example application
kubectl apply -f k8s-manifests/examples/

# Check status
kubectl get pods,canaries,scaledobjects
```

## 📁 Structure

```
dotconfig/
├── docker-compose.yml              # Local dev environment
├── scripts/nu/platform/          # Platform management scripts
│   └── stack.nu                 # Platform component installers
├── config/                       # Configuration files
│   ├── grafana/                  # Grafana dashboards & provisioning
│   ├── prometheus/               # Prometheus config & alerts
│   ├── loki/                    # Log aggregation
│   └── promtail/                # Log collector
├── k8s-manifests/               # Kubernetes manifests
│   ├── examples/                 # Example configurations
│   │   ├── dapr-flagsmith/      # Dapr + Flagsmith integration
│   │   ├── canary-deployment/   # Flagger + KEDA examples
│   │   ├── chaos/               # Chaos experiments
│   │   └── crossplane/          # Cloud resource definitions
│   └── keda/                    # KEDA scaler examples
├── src/                          # Rust application source
├── .github/                      # CI/CD workflows
│   └── workflows/               # GitHub Actions
├── tests/                        # Integration tests
└── docs/                         # Documentation
```

## 🎯 Platform Components

### Service Mesh & Progressive Delivery
- **Istio** - Service mesh with mTLS and traffic management
- **Flagger** - Progressive delivery with canary deployments
- **KEDA** - Event-driven autoscaling

### Feature Flags
- **Flagsmith** - Feature flag management
- **Dapr** - Distributed application runtime

### Observability
- **Prometheus** - Metrics collection
- **Grafana** - Dashboards and visualization
- **Jaeger** - Distributed tracing
- **Loki** - Log aggregation
- **Kiali** - Service mesh observability

### Chaos Engineering
- **Chaos Mesh** - Chaos testing platform
- Various experiments: pod, network, IO, stress

### Cloud Resources
- **Crossplane** - Cloud resource management
- Support for AWS, GCP, Azure resources

### CI/CD
- **GitHub Actions** - Automated pipelines
- **Tekton** - Kubernetes-native pipelines
- **Argo Workflows** - Workflow automation

### Messaging & Data
- **NATS** - Message queue with JetStream
- **Redis** - State store
- **MinIO** - Object storage
- **ClickHouse** - Data warehouse

## 📚 Documentation

### Getting Started
- [Platform Quick Start](PLATFORM_QUICKSTART.md) - 5 minute quick start
- [Local Development](LOCAL_DEV.md) - Local dev guide
- [Platform Guide](PLATFORM.md) - Complete platform documentation

### Examples & Configurations
- [Example Manifests](k8s-manifests/examples/README.md) - Kubernetes examples
- [Chaos Experiments](k8s-manifests/examples/chaos/README.md) - Chaos scenarios
- [CI/CD Workflows](.github/CI-CD.md) - Pipeline documentation

### Component Guides
- [KEDA Scalers](k8s-manifests/keda/README.md) - Autoscaling examples
- [Dapr Integration](k8s-manifests/examples/dapr-flagsmith/) - Dapr components
- [Canary Deployment](k8s-manifests/examples/canary-deployment/) - Flagger examples

## 🎮 Common Commands

### Platform Management

```bash
# Check platform status
nu scripts/nu/platform/stack.nu status

# Install specific components
nu scripts/nu/platform/stack.nu install-component istio
nu scripts/nu/platform/stack.nu install-component flagsmith
nu scripts/nu/platform/stack.nu install-component chaos-mesh

# Install all components
nu scripts/nu/platform/stack.nu install-all

# Uninstall components
nu scripts/nu/platform/stack.nu uninstall-component chaos-mesh
```

### Feature Flags

```bash
# Access Flagsmith UI
open http://localhost:3000

# Test feature flags
curl http://localhost:8080/feature-flags
```

### Progressive Deployment

```bash
# Trigger canary deployment
kubectl set image deployment/dotconfig dotconfig=v2.0

# Monitor canary progress
kubectl get canary dotconfig -w
```

### Chaos Testing

```bash
# Apply chaos experiments
kubectl apply -f k8s-manifests/examples/chaos/

# Monitor resilience
kubectl logs -f deployment/dotconfig
```

### Observability

```bash
# View metrics
open http://localhost:9090

# View dashboards
open http://localhost:3001

# View traces
open http://localhost:16686

# View service mesh
open http://localhost:20001/kiali
```

## 🔧 Configuration

### Environment Variables

Edit `docker-compose.yml` to customize:

```yaml
environment:
  - RUST_LOG=info
  - FLAGSMITH_API_URL=http://flagsmith-api:8000/api/v1
  - FLAGSMITH_ENV_KEY=local
```

### Platform Stack Script

The `scripts/nu/platform/stack.nu` script provides a layered installation approach:

- **Layer 0**: Core infrastructure (Istio, Gateway API)
- **Layer 1**: GitOps & CI/CD (FluxCD, Crossplane, Tekton)
- **Layer 2**: Progressive delivery (Flagger, Argo Rollouts)
- **Layer 3**: Data & Integration (Dapr)
- **Layer 4**: Observability (Prometheus, Grafana, Kiali, Jaeger)
- **Layer 5**: Chaos Engineering (Chaos Mesh)
- **Layer 6**: Data Platform (MinIO, ClickHouse, NATS)
- **Layer 7**: Feature Flags (Flagsmith)

## 🧪 Testing

```bash
# Run unit tests
cargo test

# Run integration tests
kubectl apply -f tests/kubetest/

# Run load tests
k6 run tests/k6/full-stack.test.js

# Run chaos tests
kubectl apply -f k8s-manifests/examples/chaos/
```

## 📊 Monitoring & Debugging

### Check Application Health

```bash
# Pod status
kubectl get pods -l app=dotconfig

# Application logs
kubectl logs -f deployment/dotconfig

# Dapr sidecar logs
kubectl logs -f deployment/dotconfig -c daprd

# Health check
curl http://localhost:8080/health
```

### Check Platform Components

```bash
# Platform status
nu scripts/nu/platform/stack.nu status

# Component logs
kubectl logs -n istio-system deployment/istiod
kubectl logs -n monitoring deployment/prometheus-stack-kube-prom-operator
kubectl logs -n chaos-mesh deployment/chaos-controller-manager
```

### Metrics & Traces

```bash
# Application metrics
curl http://localhost:8080/metrics

# Dapr metrics
curl http://localhost:3500/v1.0/metrics

# Prometheus metrics
curl http://localhost:9090/api/v1/query?query=up
```

## 🔄 CI/CD

### GitHub Actions Pipeline

The CI/CD pipeline includes:
- Automated testing (unit, integration, chaos)
- Multi-arch Docker builds
- Vulnerability scanning
- Staging deployment with chaos tests
- Production canary deployments

```bash
# View pipeline status
gh run list

# View specific run
gh run view <run-id>

# Trigger manual workflow
gh workflow run ci-cd.yml
```

### Tekton Pipeline

Kubernetes-native pipeline with:
- Git triggers
- Build, deploy, test, chaos stages
- Argo Workflow integration

## 🛠️ Troubleshooting

### Services Not Starting

```bash
# Docker Compose
docker compose logs
kubectl get events

# Kubernetes
kubectl describe pod <pod-name>
kubectl get events --field-selector involvedObject.kind=Pod
```

### Port Conflicts

Change ports in `docker-compose.yml`:

```yaml
ports:
  - "3002:3000"  # Change Grafana to 3002
```

### Chaos Experiments Running Too Long

```bash
# Pause chaos
kubectl patch podchaos dotconfig-pod-failure -p '{"spec":{"pause":true}}'

# Delete chaos
kubectl delete podchaos, networkchaos --all

# Restart deployment
kubectl rollout restart deployment/dotconfig
```

### Canary Deployment Stuck

```bash
# Check Flagger logs
kubectl logs -n istio-system deployment/flagger

# Check canary status
kubectl get canary dotconfig -o yaml

# Describe canary
kubectl describe canary dotconfig
```

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📜 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🔗 Resources

- [Dapr Documentation](https://docs.dapr.io/)
- [Istio Documentation](https://istio.io/latest/docs/)
- [Flagger Documentation](https://flagger.app/)
- [Flagsmith Documentation](https://docs.flagsmith.com/)
- [KEDA Documentation](https://keda.sh/docs/)
- [Chaos Mesh Documentation](https://chaos-mesh.org/docs/)
- [Crossplane Documentation](https://docs.crossplane.io/)
- [FluxCD Documentation](https://fluxcd.io/docs/)

## 📞 Support

- Issues: [GitHub Issues](https://github.com/yurikrupnik/dotconfig/issues)
- Discussions: [GitHub Discussions](https://github.com/yurikrupnik/dotconfig/discussions)
