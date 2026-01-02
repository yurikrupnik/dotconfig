# Enterprise Cloud-Native Platform

Complete cloud-native platform stack for dotconfig with comprehensive observability, feature flags, chaos engineering, and progressive delivery.

## Platform Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      User/GitOps Interface                      │
│  (FluxCD, GitHub, CI/CD)                                         │
└─────────────────────────┬───────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Istio      │  │   Crossplane │  │   FluxCD     │
│ (Service     │  │ (Cloud       │  │ (GitOps      │
│  Mesh)       │  │  Resources)  │  │  Delivery)   │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       └─────────────────┼─────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   Dapr       │  │   Flagsmith  │  │   Flagger    │
│ (Distributed │  │ (Feature     │  │ (Progressive │
│  Runtime)    │  │  Flags)      │  │  Delivery)   │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                │                │
       └────────────────┼────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   KEDA       │  │   NATS       │  │   Chaos      │
│ (Autoscaling)│  │ (Message     │  │   Mesh       │
│              │  │  Queue)      │  │ (Chaos Eng)  │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                │                │
       └────────────────┼────────────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
        ▼                ▼                ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Prometheus  │  │   Jaeger     │  │   Grafana    │
│  + Grafana   │  │ (Tracing)    │  │ (Viz)        │
└──────────────┘  └──────────────┘  └──────────────┘
```

## Components

### 🚀 Core Infrastructure

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **Istio** | Service mesh with mTLS, traffic management | `istio-system` |
| **Gateway API** | Kubernetes Gateway API CRDs | `default` |
| **Flagger** | Progressive delivery with canary deployments | `istio-system` |
| **FluxCD** | GitOps continuous delivery | `flux-system` |
| **Tekton** | Cloud-native CI/CD pipelines | `tekton-pipelines` |

### 🎯 Feature Management

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **Flagsmith** | Feature flag management | `flagsmith` |
| **Dapr** | Distributed application runtime | `dapr-system` |

### 📊 Observability

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **Prometheus** | Metrics collection and alerting | `monitoring` |
| **Grafana** | Visualization and dashboards | `monitoring` |
| **Kiali** | Istio service mesh observability | `istio-system` |
| **Jaeger** | Distributed tracing | `observability` |
| **Loki** | Log aggregation | `loki` |
| **Promtail** | Log collection agent | `loki` |

### 🔧 Data & Messaging

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **NATS** | Message queue with JetStream | `data` |
| **MinIO** | S3-compatible object storage | `data` |
| **ClickHouse** | OLAP data warehouse | `data` |
| **Redis** | Caching and Dapr state store | `data` |

### 🎲 Chaos Engineering

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **Chaos Mesh** | Chaos engineering platform | `chaos-mesh` |

### ☁️ Cloud Resources

| Component | Purpose | Namespace |
|-----------|---------|-----------|
| **Crossplane** | Cloud resource management and control plane | `crossplane-system` |

## Quick Start

### Local Development with Docker Compose

```bash
# Start all services locally
docker compose up -d

# Check service status
docker compose ps

# View logs
docker compose logs -f dotconfig-app

# Access services
# - Application: http://localhost:8080
# - Flagsmith API: http://localhost:8000
# - Flagsmith UI: http://localhost:3000
# - Grafana: http://localhost:3001 (admin/admin)
# - Prometheus: http://localhost:9090
# - Jaeger: http://localhost:16686
# - Chaos Dashboard: http://localhost:2333
```

### Kubernetes Deployment

```bash
# Install entire platform stack
nu scripts/nu/platform/stack.nu install-all

# Install specific components
nu scripts/nu/platform/stack.nu install-component istio
nu scripts/nu/platform/stack.nu install-component dapr
nu scripts/nu/platform/stack.nu install-component flagger
nu scripts/nu/platform/stack.nu install-component flagsmith

# Check platform status
nu scripts/nu/platform/stack.nu status
```

### Deploy Application with All Integrations

```bash
# Deploy Dapr components (pubsub, state store, feature flags)
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/dapr-components.yaml

# Deploy application with Dapr sidecar
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/deployment.yaml

# Deploy Flagger canary configuration
kubectl apply -f k8s-manifests/examples/canary-deployment/canary.yaml

# Deploy KEDA autoscaling
kubectl apply -f k8s-manifests/examples/canary-deployment/scaledobject.yaml

# Deploy Chaos Mesh experiments
kubectl apply -f k8s-manifests/examples/chaos/experiments.yaml

# Deploy Crossplane resources
kubectl apply -f k8s-manifests/examples/crossplane/resources.yaml
```

## Usage Examples

### 1. Feature Flags with Flagsmith

```bash
# Access Flagsmith UI
open http://localhost:3000

# Create a feature flag
# Name: new_feature
# Type: Boolean
# Default: True

# Use in application
curl http://localhost:8080/feature-flags
```

### 2. Progressive Deployment with Flagger

```bash
# Trigger a canary deployment
kubectl set image deployment/dotconfig dotconfig=yurikrupnik/dotconfig:v2.0

# Monitor canary progress
kubectl get canary dotconfig -w

# View Flagger events
kubectl get events --field-selector involvedObject.kind=Canary
```

### 3. Autoscaling with KEDA

```bash
# Check scaled object status
kubectl get scaledobject dotconfig-scaledobject

# View HPA created by KEDA
kubectl get hpa

# Trigger scaling
hey -n 10000 -c 50 http://dotconfig:8080/
```

### 4. Chaos Engineering

```bash
# List chaos experiments
kubectl get podchaos, networkchaos, iochaos, stresschaos

# Create pod failure chaos
kubectl apply -f k8s-manifests/examples/chaos/pod-failure.yaml

# Monitor application resilience
kubectl logs -f deployment/dotconfig
```

### 5. Observability

```bash
# View metrics in Prometheus
open http://localhost:9090

# View dashboards in Grafana
open http://localhost:3001

# View traces in Jaeger
open http://localhost:16686

# View service mesh in Kiali
open http://localhost:20001/kiali

# Check logs in Loki
curl http://localhost:3100/loki/api/v1/query
```

### 6. Cloud Resources with Crossplane

```bash
# View managed resources
kubectl get managed

# Check S3 bucket status
kubectl get bucket

# Check RDS instance
kubectl get instance.rds.aws.crossplane.io

# Create a cloud resource
kubectl apply -f k8s-manifests/examples/crossplane/s3-bucket.yaml
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Application log level | `info` |
| `DAPR_HTTP_PORT` | Dapr HTTP sidecar port | `3500` |
| `DAPR_GRPC_PORT` | Dapr gRPC sidecar port | `50001` |
| `FLAGSMITH_API_URL` | Flagsmith API URL | `http://localhost:8000` |
| `FLAGSMITH_ENV_KEY` | Flagsmith environment key | `local` |
| `NATS_URL` | NATS server URL | `nats://localhost:4222` |
| `REDIS_URL` | Redis server URL | `redis://localhost:6379` |

### Dapr Configuration

```yaml
apiVersion: dapr.io/v1alpha1
kind: Configuration
metadata:
  name: dapr-config
spec:
  tracing:
    samplingRate: "1"
    zipkin:
      endpointAddress: "http://jaeger-collector.istio-system:9411/api/v2/spans"
  metrics:
    enabled: true
  features:
    - name: Resiliency
      enabled: true
```

### Istio Configuration

```yaml
apiVersion: networking.istio.io/v1alpha3
kind: DestinationRule
metadata:
  name: dotconfig
spec:
  host: dotconfig
  trafficPolicy:
    connectionPool:
      tcp:
        maxConnections: 100
    loadBalancer:
      simple: LEAST_CONN
    circuitBreaker:
      consecutiveErrors: 3
      interval: 30s
      baseEjectionTime: 30s
```

## Monitoring & Debugging

### Check Component Status

```bash
# All platform components
nu scripts/nu/platform/stack.nu status

# Specific component logs
kubectl logs -n istio-system deployment/istiod
kubectl logs -n dapr-system deployment/dapr-operator
kubectl logs -n monitoring deployment/prometheus-stack-kube-prom-operator
kubectl logs -n chaos-mesh deployment/chaos-controller-manager
```

### Application Logs

```bash
# View application logs
kubectl logs -f deployment/dotconfig

# View Dapr sidecar logs
kubectl logs -f deployment/dotconfig -c daprd

# View NATS messages
docker logs -f nats
```

### Metrics

```bash
# Query Prometheus metrics
curl http://localhost:9090/api/v1/query?query=up

# View custom application metrics
curl http://localhost:8080/metrics

# Check Dapr metrics
curl http://localhost:3500/v1.0/metrics
```

## Troubleshooting

### Common Issues

**Issue: Pods stuck in CrashLoopBackOff**
```bash
# Check pod logs
kubectl describe pod <pod-name>
kubectl logs <pod-name> --previous

# Check resource limits
kubectl describe deployment <deployment-name>
```

**Issue: Flagger canary not progressing**
```bash
# Check Flagger logs
kubectl logs -n istio-system deployment/flagger

# Check canary status
kubectl describe canary <canary-name>

# Check HPA status
kubectl get hpa
```

**Issue: KEDA not scaling**
```bash
# Check KEDA operator logs
kubectl logs -n keda -l app=keda-operator

# Check metrics server
kubectl get --raw /apis/metrics.k8s.io/v1beta1/namespaces/default/pods

# Check ScaledObject events
kubectl describe scaledobject <scaledobject-name>
```

**Issue: Dapr sidecar not working**
```bash
# Check Dapr installation
dapr status -k

# Check sidecar injection
kubectl get deployment <deployment-name> -o yaml | grep dapr

# Test Dapr invocation
dapr invoke --app-id <app-id> --method <method>
```

### Health Checks

```bash
# Application health
curl http://localhost:8080/health

# Dapr health
curl http://localhost:3500/v1.0/healthz

# Prometheus health
curl http://localhost:9090/-/healthy

# Grafana health
curl http://localhost:3001/api/health

# NATS health
curl http://localhost:8222/varz
```

## Security Considerations

- **mTLS**: Enabled by default in Istio
- **RBAC**: Service accounts and role bindings configured
- **Secrets**: Use Kubernetes secrets for sensitive data
- **Network Policies**: Implement network segmentation
- **Image Security**: Use signed and scanned images
- **Audit Logging**: Enabled in all components

## Performance Tuning

### Resource Limits

```yaml
resources:
  requests:
    memory: "128Mi"
    cpu: "100m"
  limits:
    memory: "512Mi"
    cpu: "500m"
```

### Autoscaling

- **KEDA**: Event-driven scaling based on queue length, HTTP requests, etc.
- **HPA**: Traditional resource-based scaling
- **Flagger**: Canary rollouts with automatic traffic shifting

## Backup & Recovery

```bash
# Backup configuration
kubectl get configmaps -A -o yaml > configmaps-backup.yaml
kubectl get secrets -A -o yaml > secrets-backup.yaml

# Backup Crossplane managed resources
kubectl get managed -A -o yaml > managed-resources.yaml

# Backup Dapr state
redis-cli --rdb /backup/dump.rdb
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Resources

- [Dapr Documentation](https://docs.dapr.io/)
- [Istio Documentation](https://istio.io/latest/docs/)
- [Flagger Documentation](https://flagger.app/)
- [Flagsmith Documentation](https://docs.flagsmith.com/)
- [KEDA Documentation](https://keda.sh/docs/)
- [Chaos Mesh Documentation](https://chaos-mesh.org/docs/)
- [Crossplane Documentation](https://docs.crossplane.io/)
- [FluxCD Documentation](https://fluxcd.io/docs/)
