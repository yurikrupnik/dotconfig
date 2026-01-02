# KEDA (Kubernetes Event-Driven Autoscaling)

Event-driven autoscaling for dotconfig workloads.

## Overview

KEDA extends Kubernetes with event-driven autoscaling capabilities:

- Scale deployments from 0 to N based on events
- Scale based on metrics from external sources
- Support for 50+ scalers (Redis, Kafka, Prometheus, Cron, HTTP, etc.)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    KEDA Components                           │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Operator  │  │   Metrics   │  │   Admission         │  │
│  │  Controller │  │   Server    │  │   Webhooks          │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘  │
│         │                │                                   │
│         ▼                ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              ScaledObject / ScaledJob                   ││
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    ││
│  │  │  Redis  │  │Prometheus│  │  Cron   │  │  HTTP   │    ││
│  │  │ Scaler  │  │ Scaler  │  │ Scaler  │  │ Scaler  │    ││
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘    ││
│  └───────┼────────────┼────────────┼────────────┼──────────┘│
└──────────┼────────────┼────────────┼────────────┼───────────┘
           │            │            │            │
           ▼            ▼            ▼            ▼
     ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
     │  Redis   │  │Prometheus│  │  Timer   │  │  HTTP    │
     │  Queue   │  │ Metrics  │  │          │  │ Requests │
     └──────────┘  └──────────┘  └──────────┘  └──────────┘
```

## Installation

```bash
# Install KEDA
helm repo add kedacore https://kedacore.github.io/charts
helm repo update
helm install keda kedacore/keda --namespace keda --create-namespace

# Or with kubectl
kubectl apply -f https://github.com/kedacore/keda/releases/download/v2.13.0/keda-2.13.0.yaml
```

## Scalers Included

| Scaler | Use Case | File |
|--------|----------|------|
| **Prometheus** | Scale on custom metrics | `prometheus-scaler.yaml` |
| **Redis** | Scale on queue length | `redis-scaler.yaml` |
| **Cron** | Time-based scaling | `cron-scaler.yaml` |
| **HTTP** | Scale on HTTP traffic | `http-scaler.yaml` |
| **CPU/Memory** | Resource-based scaling | `resource-scaler.yaml` |
| **External** | Custom external metrics | `external-scaler.yaml` |

## Usage

```bash
# Apply all KEDA resources
kubectl apply -f k8s-manifests/keda/

# Check scaled objects
kubectl get scaledobjects
kubectl get scaledjobs

# Check HPA created by KEDA
kubectl get hpa

# View scaling activity
kubectl describe scaledobject dotconfig-scaler
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `KEDA_MIN_REPLICAS` | Minimum replicas | 1 |
| `KEDA_MAX_REPLICAS` | Maximum replicas | 10 |
| `KEDA_COOLDOWN_PERIOD` | Cooldown after scale down | 300 |
| `KEDA_POLLING_INTERVAL` | How often to check metrics | 30 |

## Monitoring

```bash
# KEDA operator logs
kubectl logs -n keda -l app=keda-operator

# Metrics server logs
kubectl logs -n keda -l app=keda-metrics-apiserver

# ScaledObject status
kubectl get scaledobject -o wide
```
