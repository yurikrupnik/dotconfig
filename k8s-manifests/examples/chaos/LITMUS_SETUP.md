# Litmus Chaos Setup Guide

This guide helps you set up Litmus Chaos alongside or instead of Chaos Mesh.

## Table of Contents
1. [Installation](#installation)
2. [Comparison with Chaos Mesh](#comparison-with-chaos-mesh)
3. [Quick Start](#quick-start)
4. [Running Experiments](#running-experiments)
5. [Migration from Chaos Mesh](#migration-from-chaos-mesh)
6. [Troubleshooting](#troubleshooting)

## Installation

### Prerequisites
- Kubernetes cluster (1.17+)
- kubectl configured
- Helm 3.x (optional but recommended)

### Option 1: Helm Installation (Recommended)

```bash
# Add Litmus Helm repo
helm repo add litmuschaos https://litmuschaos.github.io/litmus-helm/
helm repo update

# Install Litmus
kubectl create namespace litmus
helm install litmus litmuschaos/litmus \
  --namespace litmus \
  --set portal.frontend.service.type=LoadBalancer

# Wait for pods to be ready
kubectl wait --for=condition=Ready pods --all -n litmus --timeout=300s
```

### Option 2: Kubectl Installation

```bash
# Install Litmus CRDs and operator
kubectl apply -f https://litmuschaos.github.io/litmus/3.0.0/litmus-3.0.0.yaml

# Verify installation
kubectl get pods -n litmus
kubectl get crds | grep chaos
```

### Install ChaosHub Experiments

```bash
# Install generic experiments
kubectl apply -f https://hub.litmuschaos.io/api/chaos/3.0.0?file=charts/generic/experiments.yaml -n litmus

# Or install all experiments
kubectl apply -f https://hub.litmuschaos.io/api/chaos/master?file=charts/generic/experiments.yaml
```

## Comparison with Chaos Mesh

| Feature | Chaos Mesh | Litmus | Winner |
|---------|-----------|--------|--------|
| **Ease of Use** | Simple YAML | More complex | Chaos Mesh |
| **Performance** | Kernel-level | Container-level | Chaos Mesh |
| **Workflow Orchestration** | Basic scheduler | Advanced workflows | Litmus |
| **GitOps Integration** | Good | Excellent | Litmus |
| **Observability** | Built-in dashboard | Litmus Portal | Tie |
| **Hypothesis Testing** | No | Yes (probes) | Litmus |
| **Pod Chaos** | ✅ | ✅ | Tie |
| **Network Chaos** | ✅ Advanced | ✅ Good | Chaos Mesh |
| **Stress Testing** | ✅ | ✅ | Tie |
| **IO Chaos** | ✅ Advanced | ⚠️ Limited | Chaos Mesh |
| **Kernel Chaos** | ✅ | ❌ | Chaos Mesh |
| **Time Chaos** | ✅ | ❌ | Chaos Mesh |
| **HTTP Chaos** | ✅ | ✅ (via toxiproxy) | Chaos Mesh |
| **DNS Chaos** | ✅ Advanced | ✅ Basic | Chaos Mesh |
| **Community** | CNCF Sandbox | CNCF Graduated | Litmus |
| **Pre-built Experiments** | Limited | ChaosHub library | Litmus |

## Quick Start

### 1. Install Required RBAC for Examples

```bash
# Apply RBAC from litmus-pod-chaos.yaml
kubectl apply -f k8s-manifests/examples/chaos/litmus-pod-chaos.yaml
```

### 2. Verify Installation

```bash
# Check if experiments are installed
kubectl get chaosexperiments -A

# Check if chaos operator is running
kubectl get pods -n litmus
```

### 3. Run Your First Experiment

```bash
# Apply pod-delete chaos
kubectl apply -f - <<EOF
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: test-chaos
  namespace: default
spec:
  annotationCheck: "false"
  engineState: "active"
  appinfo:
    appns: default
    applabel: "app=dotconfig"
    appkind: deployment
  chaosServiceAccount: pod-delete-sa
  experiments:
  - name: pod-delete
    spec:
      components:
        env:
        - name: TOTAL_CHAOS_DURATION
          value: "30"
        - name: PODS_AFFECTED_PERC
          value: "50"
EOF

# Watch chaos progress
kubectl get chaosengine -n default -w
kubectl get chaosresult -n default
```

## Running Experiments

### Available Litmus Equivalents

All Litmus equivalents for your Chaos Mesh experiments are in:
- `litmus-pod-chaos.yaml` - Pod kill, container kill, pod failure
- `litmus-network-chaos.yaml` - Network partition, loss, duplication, corruption, latency
- `litmus-stress-chaos.yaml` - CPU, memory stress testing
- `litmus-io-chaos.yaml` - IO stress, disk fill (limited compared to Chaos Mesh)
- `litmus-advanced-chaos.yaml` - HTTP chaos (Note: No kernel/time chaos)

### Run All Pod Chaos Experiments

```bash
kubectl apply -f k8s-manifests/examples/chaos/litmus-pod-chaos.yaml
```

### Run Specific Experiment

```bash
# Just run network partition
kubectl apply -f - <<EOF
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: network-partition-test
  namespace: default
spec:
  annotationCheck: "false"
  engineState: "active"
  appinfo:
    appns: default
    applabel: "app=dotconfig"
    appkind: deployment
  chaosServiceAccount: pod-delete-sa
  jobCleanUpPolicy: "delete"
  experiments:
  - name: pod-network-partition
    spec:
      components:
        env:
        - name: TOTAL_CHAOS_DURATION
          value: "30"
        - name: DESTINATION_HOSTS
          value: "redis"
EOF
```

### Monitor Experiments

```bash
# View active chaos engines
kubectl get chaosengine -A

# View experiment results
kubectl get chaosresult -A

# Check detailed results
kubectl describe chaosresult <result-name> -n default

# View logs
kubectl logs -l app.kubernetes.io/component=experiment-job -n default
```

### Stop Experiments

```bash
# Stop a specific chaos engine
kubectl patch chaosengine <engine-name> -n default \
  --type merge \
  --patch '{"spec":{"engineState":"stop"}}'

# Delete chaos engine
kubectl delete chaosengine <engine-name> -n default
```

## Migration from Chaos Mesh

### Step 1: Side-by-Side Testing

Run both Chaos Mesh and Litmus in parallel:

```bash
# Both can coexist - they use different CRDs
kubectl get crds | grep chaos

# Chaos Mesh CRDs
podchaos.chaos-mesh.org
networkchaos.chaos-mesh.org

# Litmus CRDs
chaosengines.litmuschaos.io
chaosexperiments.litmuschaos.io
```

### Step 2: Feature Parity Check

| Your Chaos Mesh Experiments | Litmus Equivalent | Notes |
|----------------------------|-------------------|-------|
| pod-chaos.yaml | litmus-pod-chaos.yaml | ✅ Full parity |
| network-chaos.yaml | litmus-network-chaos.yaml | ✅ Good parity |
| stress-chaos.yaml | litmus-stress-chaos.yaml | ✅ Full parity |
| io-chaos.yaml | litmus-io-chaos.yaml | ⚠️ Limited (no latency/errno injection) |
| advanced-chaos.yaml | litmus-advanced-chaos.yaml | ⚠️ No kernel/time chaos |

### Step 3: Recommended Approach

**Option A: Full Migration to Litmus** ✅ If you:
- Don't use Kernel/Time chaos
- Want better workflow orchestration
- Need GitOps integration
- Want hypothesis-driven testing

**Option B: Keep Chaos Mesh** ✅ If you:
- Heavily use IO chaos with latency injection
- Need kernel-level fault injection
- Require time chaos
- Prefer simpler YAML

**Option C: Hybrid (Recommended)** ✅ Use:
- Chaos Mesh for: Kernel chaos, time chaos, advanced IO
- Litmus for: Workflow orchestration, hypothesis testing, standard chaos

### Step 4: Uninstall Chaos Mesh (if migrating fully)

```bash
# Remove Chaos Mesh if you decide to go Litmus-only
helm uninstall chaos-mesh -n chaos-mesh

# Or via kubectl
kubectl delete -f https://mirrors.chaos-mesh.org/latest/crd.yaml
kubectl delete namespace chaos-mesh
```

## Advanced Features

### Using Probes (Hypothesis Testing)

Litmus allows you to validate hypotheses using probes:

```yaml
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: pod-delete-with-probe
  namespace: default
spec:
  appinfo:
    appns: default
    applabel: "app=dotconfig"
    appkind: deployment
  chaosServiceAccount: pod-delete-sa
  experiments:
  - name: pod-delete
    spec:
      probe:
      - name: "check-service-availability"
        type: "httpProbe"
        mode: "Continuous"
        httpProbe/inputs:
          url: "http://dotconfig-service:8080/health"
          insecureSkipVerify: false
          method:
            get:
              criteria: "=="
              responseCode: "200"
        runProperties:
          probeTimeout: 5
          interval: 2
          retry: 1
```

### Workflow with Argo

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Workflow
metadata:
  name: chaos-workflow
  namespace: litmus
spec:
  entrypoint: chaos-test
  templates:
  - name: chaos-test
    steps:
    - - name: install-chaos
        template: install-chaos-experiment
    - - name: run-chaos
        template: run-chaos-experiment
    - - name: verify-results
        template: verify-chaos-results
```

### GitOps with Flux/ArgoCD

```bash
# Store chaos experiments in Git
git add k8s-manifests/examples/chaos/litmus-*.yaml
git commit -m "Add Litmus chaos experiments"
git push

# Let GitOps tool sync
flux reconcile kustomization chaos-experiments
# or
argocd app sync chaos-experiments
```

## Troubleshooting

### Common Issues

**1. ChaosEngine stuck in "Running"**
```bash
# Check runner pod
kubectl get pods -l chaosUID=<chaos-engine-uid> -n default
kubectl logs <runner-pod> -n default

# Check RBAC
kubectl auth can-i create pods --as=system:serviceaccount:default:pod-delete-sa
```

**2. Experiments not found**
```bash
# Install missing experiments
kubectl apply -f https://hub.litmuschaos.io/api/chaos/3.0.0?file=charts/generic/experiments.yaml
```

**3. Insufficient permissions**
```bash
# Verify ServiceAccount has necessary permissions
kubectl describe role pod-delete-sa-role -n default
kubectl describe rolebinding pod-delete-sa-role-binding -n default
```

**4. CronJobs not creating chaos**
```bash
# Check CronJob status
kubectl get cronjobs -n default
kubectl describe cronjob pod-delete-chaos-cron -n default

# Check if jobs are created
kubectl get jobs -n default
```

### Debug Mode

```bash
# Enable verbose logging
kubectl set env deployment/chaos-operator-ce -n litmus \
  LOG_LEVEL=debug

# View operator logs
kubectl logs -f deployment/chaos-operator-ce -n litmus
```

### Cleanup

```bash
# Remove all chaos resources
kubectl delete chaosengines --all -n default
kubectl delete chaosresults --all -n default
kubectl delete cronjobs -l app.kubernetes.io/part-of=litmus -n default

# Uninstall Litmus
helm uninstall litmus -n litmus
kubectl delete namespace litmus
```

## Best Practices

1. **Start Small**: Test with one experiment before running all
2. **Use Probes**: Add health checks to validate system behavior
3. **Gradual Rollout**: Increase chaos intensity gradually
4. **Monitor**: Use Litmus Portal or Grafana for observability
5. **Document**: Keep notes on chaos results in Git
6. **Automate**: Use GitOps for chaos experiment lifecycle
7. **Test in Lower Envs**: Always test chaos in dev/staging first

## Resources

- [Litmus Docs](https://docs.litmuschaos.io/)
- [ChaosHub](https://hub.litmuschaos.io/)
- [Litmus GitHub](https://github.com/litmuschaos/litmus)
- [CNCF Litmus Project](https://www.cncf.io/projects/litmus/)
- [Chaos Engineering Principles](https://principlesofchaos.org/)

## Next Steps

1. Install Litmus: Follow [Installation](#installation)
2. Run first test: `kubectl apply -f k8s-manifests/examples/chaos/litmus-pod-chaos.yaml`
3. Monitor results: Use Litmus Portal or kubectl
4. Compare with Chaos Mesh: Run equivalent experiments side-by-side
5. Make decision: Choose the tool(s) that fit your needs

## Questions?

- Check the detailed notes in each `litmus-*-chaos.yaml` file
- Review `litmus-advanced-chaos-notes` ConfigMap for limitations
- Review `litmus-io-chaos-notes` ConfigMap for IO chaos gaps
