# Resource Stats Operator

Kubernetes operator that collects CPU/memory/GPU metrics, calculates costs, and exposes data via web UI, REST API, and terminal UI.

## Installation

```bash
# Build with all features
cargo build --release --features full

# Or with specific features
cargo build --release --features web-ui
cargo build --release --features tui
```

## Usage

### Run Full Operator (with CRD Controllers)

```bash
# Run controllers only
resource-stats-operator run

# Run controllers with web UI
resource-stats-operator run --with-web

# Custom web address
resource-stats-operator run --with-web --addr 0.0.0.0:3000
```

This runs the Kubernetes controllers that watch for `CostConfig` and `ResourceStats` CRDs.

### Collect Metrics Once

```bash
# Table format (default)
resource-stats-operator collect

# JSON format
resource-stats-operator collect --format json

# Save to a ResourceStats CRD
resource-stats-operator collect --save-to-crd my-stats --namespace monitoring
```

### Run Web Server (without controllers)

```bash
# Default port 8080
resource-stats-operator serve

# Custom port
resource-stats-operator serve --addr 0.0.0.0:3000
```

Then visit `http://localhost:8080` for the dashboard or use the API:

```bash
# Cluster stats
curl http://localhost:8080/api/v1/stats/cluster

# Node stats
curl http://localhost:8080/api/v1/stats/nodes

# Health check
curl http://localhost:8080/api/v1/health
```

### Run Terminal UI

```bash
resource-stats-operator tui
```

Controls:
- `Tab` / Arrow keys: Navigate between tabs
- `q`: Quit

### Generate CRD Manifests

```bash
resource-stats-operator crds > crds.yaml
kubectl apply -f crds.yaml
```

## CRDs

### CostConfig

Defines pricing for resources:

```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: CostConfig
metadata:
  name: default-pricing
  namespace: resource-stats
spec:
  source: static
  currency: USD
  staticPricing:
    cpuPerCoreHour: "0.031611"
    memoryPerGibHour: "0.004237"
    gpuPricing:
      - vendor: nvidia
        modelPattern: ".*A100.*"
        perGpuHour: "2.93"
      - vendor: nvidia
        modelPattern: ".*T4.*"
        perGpuHour: "0.35"
```

### CostConfig with Cloud Pricing

Use cloud provider APIs to fetch current pricing:

**GCP Cloud Billing:**
```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: CostConfig
metadata:
  name: gcp-pricing
spec:
  source: cloud
  cloudProvider: gcp
  region: us-central1
  currency: USD
  refreshInterval: "1h"
```

Requires `GOOGLE_APPLICATION_CREDENTIALS` environment variable or GKE workload identity.

**AWS Pricing API:**
```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: CostConfig
metadata:
  name: aws-pricing
spec:
  source: cloud
  cloudProvider: aws
  region: us-east-1
  currency: USD
  refreshInterval: "1h"
```

Requires `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` or IAM role.

**Azure Retail Prices:**
```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: CostConfig
metadata:
  name: azure-pricing
spec:
  source: cloud
  cloudProvider: azure
  region: eastus
  currency: USD
  refreshInterval: "1h"
```

Azure Retail Prices API is public and doesn't require authentication.

### CostConfig with Hybrid Pricing

Falls back to static pricing if cloud API fails:

```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: CostConfig
metadata:
  name: hybrid-pricing
spec:
  source: hybrid
  cloudProvider: gcp
  region: us-central1
  currency: USD
  staticPricing:
    cpuPerCoreHour: "0.031611"
    memoryPerGibHour: "0.004237"
    gpuPricing:
      - vendor: nvidia
        modelPattern: ".*A100.*"
        perGpuHour: "2.93"
```

### ResourceStats

Triggers metrics collection for a scope:

```yaml
apiVersion: platform.yurikrupnik.com/v1alpha1
kind: ResourceStats
metadata:
  name: cluster-stats
  namespace: resource-stats
spec:
  scope: cluster
  interval: "1m"
  collectGpu: true
  costConfigRef:
    name: default-pricing
```

## Features

| Feature | Flag | Description |
|---------|------|-------------|
| Web UI | `--features web-ui` | Axum-based web server with HTML dashboard |
| TUI | `--features tui` | Terminal UI using ratatui |
| NVIDIA GPU | `--features nvidia-gpu` | NVIDIA GPU metrics via nvml-wrapper |
| Full | `--features full` | All features enabled |

## GPU Support

### NVIDIA
Requires `nvidia-smi` in PATH or NVML library.

### AMD
Requires `rocm-smi` in PATH.

### Intel
Requires `xpu-smi` (Data Center GPUs) or `intel_gpu_top` (integrated).

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | HTML dashboard |
| `/api/v1/stats/cluster` | GET | Cluster-wide stats |
| `/api/v1/stats/nodes` | GET | All node stats |
| `/api/v1/health` | GET | Health check |
| `/api/v1/collect` | POST | Trigger on-demand collection (webhook) |

### Webhook API

Trigger on-demand metrics collection:

```bash
# Collect and return metrics
curl -X POST http://localhost:8080/api/v1/collect \
  -H "Content-Type: application/json" \
  -d '{}'

# Collect and save to CRD
curl -X POST http://localhost:8080/api/v1/collect \
  -H "Content-Type: application/json" \
  -d '{"save_to_crd": "my-stats", "namespace": "default"}'
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Log level (e.g., `resource_stats=debug,kube=warn`) |
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to GCP service account JSON (for GCP pricing) |
| `GOOGLE_PROJECT_ID` or `GCP_PROJECT_ID` | GCP project ID (for GCP pricing) |
| `AWS_ACCESS_KEY_ID` | AWS access key (for AWS pricing) |
| `AWS_SECRET_ACCESS_KEY` | AWS secret key (for AWS pricing) |
| `AZURE_SUBSCRIPTION_ID` | Azure subscription ID (optional, for Azure pricing) |

## Cloud Pricing Support

The operator supports fetching pricing from cloud provider APIs:

| Provider | API Used | Authentication |
|----------|----------|----------------|
| GCP | Cloud Billing API | Service account with `roles/billing.viewer` |
| AWS | Pricing API | IAM credentials with `pricing:GetProducts` |
| Azure | Retail Prices API | Public (no auth required) |

**Instance Types Supported:**

- **GCP**: N2, E2 (standard, highmem), A2 (A100 GPU), G2 (L4 GPU)
- **AWS**: M5, M6i, C5, R5, P3 (V100), P4d (A100), G4dn (T4)
- **Azure**: D-series v5, E-series v5, F-series v2, NC v3 (V100), ND A100, NC T4

Note: Cloud pricing APIs currently return fallback rates. Full API integration is planned.
