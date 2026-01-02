# Litmus Namespace Setup Guide

Complete guide for setting up Litmus Chaos in Kubernetes with proper namespace configuration.

## Quick Start

### 1. Create Namespaces and RBAC
```bash
# Create litmus namespace and RBAC
kubectl apply -f litmus-namespace.yaml

# Verify namespace creation
kubectl get namespace litmus chaos-testing
kubectl get sa -n litmus
kubectl get clusterrole litmus-admin
kubectl get clusterrolebinding litmus-admin
```

### 2. Install Litmus Operator and CRDs
```bash
# Install CRDs and operator
kubectl apply -f litmus-install.yaml

# Wait for operator to be ready
kubectl wait --for=condition=Ready pod -l app.kubernetes.io/component=operator -n litmus --timeout=300s

# Check operator status
kubectl get pods -n litmus
kubectl logs -l app.kubernetes.io/component=operator -n litmus
```

### 3. Install Chaos Experiments
```bash
# Install standard experiments
kubectl apply -f litmus-experiments.yaml

# Verify experiments are installed
kubectl get chaosexperiments -n litmus
```

### 4. Verify Installation
```bash
# Check all components
kubectl get all -n litmus

# Check CRDs
kubectl get crds | grep litmuschaos

# Check RBAC
kubectl auth can-i create pods --as=system:serviceaccount:litmus:litmus-admin -n default
```

## Complete Installation (Using Helm - Recommended)

For production use, Helm is recommended:

```bash
# Add Litmus Helm repository
helm repo add litmuschaos https://litmuschaos.github.io/litmus-helm/
helm repo update

# Create namespace
kubectl apply -f litmus-namespace.yaml

# Install Litmus with Helm
helm install litmus litmuschaos/litmus \
  --namespace litmus \
  --set portal.frontend.service.type=LoadBalancer \
  --set mongodb.persistence.size=20Gi

# Wait for all pods to be ready
kubectl wait --for=condition=Ready pods --all -n litmus --timeout=600s

# Get Litmus Portal URL
kubectl get svc -n litmus litmus-frontend-service
```

## Namespace Structure

This setup creates the following namespaces:

### 1. **litmus** (Main Namespace)
- Litmus operator and control plane
- Chaos experiments definitions
- Chaos exporter for metrics
- Litmus portal (if installed)

**Resources:**
- ServiceAccount: `litmus-admin`
- Deployment: `chaos-operator`
- Deployment: `chaos-exporter`
- ConfigMap: `litmus-config`

### 2. **chaos-testing** (Target Namespace)
- Dedicated namespace for running chaos experiments
- Pre-configured with RBAC
- ServiceAccount: `litmus-chaos-runner`

### 3. **default** (Optional Target)
- Also configured with RBAC for chaos experiments
- ServiceAccount: `litmus-chaos-runner`

## RBAC Configuration

### ClusterRole: litmus-admin
Permissions for Litmus operator:
- ✅ Manage ChaosEngines, ChaosExperiments, ChaosResults
- ✅ Manage Pods, Deployments, StatefulSets
- ✅ Create and manage Jobs
- ✅ Read Nodes (for node-level chaos)
- ✅ Manage NetworkPolicies (for network chaos)

### Role: litmus-chaos-runner
Permissions for chaos experiment runners:
- ✅ Manage pods in target namespace
- ✅ Create Jobs
- ✅ Read Deployments/StatefulSets
- ✅ Manage ChaosEngines/Results

## Resource Limits

The namespace includes optional resource quotas:

```yaml
# litmus namespace quota
requests.cpu: 4 cores
requests.memory: 8Gi
limits.cpu: 8 cores
limits.memory: 16Gi
max pods: 50
```

**Adjust these based on your cluster size:**

```bash
# Edit resource quota
kubectl edit resourcequota litmus-quota -n litmus

# Remove resource quota if not needed
kubectl delete resourcequota litmus-quota -n litmus
```

## Network Policy

A NetworkPolicy is included for security:

```yaml
# Allows:
- Ingress from same namespace
- Ingress on ports 8080, 9091, 3000 (operator, metrics, portal)
- Egress to all namespaces (chaos needs to reach targets)
- DNS egress
```

**To disable NetworkPolicy:**
```bash
kubectl delete networkpolicy litmus-network-policy -n litmus
```

## Priority Classes

Two priority classes are created:

### 1. **litmus-critical** (Priority: 1000000)
For critical operator components:
```yaml
apiVersion: v1
kind: Pod
spec:
  priorityClassName: litmus-critical
```

### 2. **litmus-chaos-job** (Priority: 100000)
For chaos experiment jobs:
```yaml
apiVersion: batch/v1
kind: Job
spec:
  template:
    spec:
      priorityClassName: litmus-chaos-job
```

## Configuration

Edit the ConfigMap to customize Litmus:

```bash
kubectl edit configmap litmus-config -n litmus
```

**Available settings:**
- `CHAOS_RUNNER_IMAGE`: Chaos runner container image
- `LOG_LEVEL`: info, debug, warn, error
- `DEFAULT_CHAOS_DURATION`: Default duration in seconds
- `JOB_CLEANUP_POLICY`: delete or retain
- `ENABLE_METRICS`: Enable Prometheus metrics

## Monitoring

### Prometheus Integration

ServiceMonitors are included for Prometheus Operator:

```bash
# Verify ServiceMonitors
kubectl get servicemonitor -n litmus

# Check metrics endpoints
kubectl port-forward -n litmus svc/chaos-operator-metrics 8080:8080
curl localhost:8080/metrics

kubectl port-forward -n litmus svc/chaos-exporter 8080:8080
curl localhost:8080/metrics
```

### Grafana Dashboards

Import Litmus dashboards:
```bash
# Download official Grafana dashboards
curl -L https://raw.githubusercontent.com/litmuschaos/litmus/master/monitoring/grafana-dashboards/litmus-dashboard.json -o litmus-dashboard.json

# Import to Grafana via UI or ConfigMap
kubectl create configmap litmus-grafana-dashboard \
  --from-file=litmus-dashboard.json \
  -n monitoring
```

## Using the Namespaces

### Run Chaos in default namespace:
```bash
kubectl apply -f - <<EOF
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: test-chaos
  namespace: default
spec:
  appinfo:
    appns: default
    applabel: "app=myapp"
    appkind: deployment
  chaosServiceAccount: litmus-chaos-runner
  experiments:
  - name: pod-delete
EOF
```

### Run Chaos in chaos-testing namespace:
```bash
# Deploy test app
kubectl create deployment nginx --image=nginx -n chaos-testing
kubectl label deployment nginx app=nginx -n chaos-testing

# Run chaos
kubectl apply -f - <<EOF
apiVersion: litmuschaos.io/v1alpha1
kind: ChaosEngine
metadata:
  name: nginx-chaos
  namespace: chaos-testing
spec:
  appinfo:
    appns: chaos-testing
    applabel: "app=nginx"
    appkind: deployment
  chaosServiceAccount: litmus-chaos-runner
  experiments:
  - name: pod-delete
EOF
```

## Add More Target Namespaces

To enable chaos in additional namespaces:

```bash
# Create namespace
kubectl create namespace production

# Create ServiceAccount and RBAC
kubectl apply -f - <<EOF
apiVersion: v1
kind: ServiceAccount
metadata:
  name: litmus-chaos-runner
  namespace: production
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: litmus-chaos-runner
  namespace: production
rules:
  - apiGroups: [""]
    resources: ["pods", "events", "pods/log", "pods/exec"]
    verbs: ["create", "list", "get", "patch", "update", "delete"]
  - apiGroups: ["batch"]
    resources: ["jobs"]
    verbs: ["create", "list", "get", "delete"]
  - apiGroups: ["litmuschaos.io"]
    resources: ["chaosengines", "chaosexperiments", "chaosresults"]
    verbs: ["create", "list", "get", "patch", "update", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: litmus-chaos-runner
  namespace: production
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: litmus-chaos-runner
subjects:
  - kind: ServiceAccount
    name: litmus-chaos-runner
    namespace: production
EOF
```

## Troubleshooting

### Operator not starting
```bash
# Check operator logs
kubectl logs -l app.kubernetes.io/component=operator -n litmus --tail=100

# Check RBAC
kubectl auth can-i list pods --as=system:serviceaccount:litmus:litmus-admin -A

# Check CRDs
kubectl get crds | grep litmuschaos
```

### Experiments failing
```bash
# Check experiment runner logs
kubectl logs -l app.kubernetes.io/component=experiment-job -n default

# Check ServiceAccount permissions
kubectl auth can-i create pods \
  --as=system:serviceaccount:default:litmus-chaos-runner \
  -n default

# Describe ChaosEngine
kubectl describe chaosengine <engine-name> -n default

# Check ChaosResult
kubectl get chaosresult -n default
kubectl describe chaosresult <result-name> -n default
```

### Network issues
```bash
# Check NetworkPolicy
kubectl get networkpolicy -n litmus
kubectl describe networkpolicy litmus-network-policy -n litmus

# Test connectivity from chaos pod
kubectl run test --image=nicolaka/netshoot -n litmus --rm -it -- bash
# Inside pod:
curl chaos-operator-metrics.litmus:8080/metrics
```

### Resource constraints
```bash
# Check resource usage
kubectl top pods -n litmus

# Check resource quota
kubectl describe resourcequota litmus-quota -n litmus

# Check if pods are pending due to resources
kubectl get pods -n litmus -o wide
kubectl describe pod <pod-name> -n litmus
```

## Cleanup

### Remove everything
```bash
# Delete all chaos experiments
kubectl delete chaosengines --all -A
kubectl delete chaosresults --all -A

# Uninstall Litmus
kubectl delete -f litmus-experiments.yaml
kubectl delete -f litmus-install.yaml
kubectl delete -f litmus-namespace.yaml

# Or if using Helm
helm uninstall litmus -n litmus
kubectl delete namespace litmus chaos-testing
```

### Keep namespace but remove experiments
```bash
# Delete only experiments
kubectl delete chaosengines --all -A
kubectl delete chaosresults --all -A
```

## Security Best Practices

1. **Limit Chaos to Specific Namespaces**
   ```bash
   # Don't give ClusterRole to chaos runners
   # Use namespace-specific Roles instead
   ```

2. **Use NetworkPolicies**
   ```bash
   # Keep the NetworkPolicy enabled
   # Add egress rules only for required targets
   ```

3. **Resource Quotas**
   ```bash
   # Always set resource quotas in production
   # Prevent chaos experiments from consuming all resources
   ```

4. **ServiceAccount Isolation**
   ```bash
   # Use separate ServiceAccounts per namespace
   # Don't share litmus-admin ServiceAccount
   ```

5. **Audit Logging**
   ```bash
   # Enable audit logging for chaos activities
   kubectl get events -n litmus --sort-by='.lastTimestamp'
   ```

## Next Steps

1. ✅ Verify installation: `kubectl get pods -n litmus`
2. ✅ Install experiments: `kubectl apply -f litmus-experiments.yaml`
3. ✅ Run first test: `kubectl apply -f litmus-pod-chaos.yaml`
4. ✅ Check results: `kubectl get chaosresult -A`
5. ✅ View metrics: `kubectl port-forward -n litmus svc/chaos-exporter 8080:8080`

## Additional Resources

- **Update ServiceAccount in existing experiments:**
  ```bash
  # Update litmus-pod-chaos.yaml to use correct ServiceAccount
  sed -i 's/pod-delete-sa/litmus-chaos-runner/g' litmus-pod-chaos.yaml
  ```

- **Namespace-aware commands:**
  ```bash
  # List all chaos engines across namespaces
  kubectl get chaosengines -A
  
  # List experiments in litmus namespace
  kubectl get chaosexperiments -n litmus
  
  # Watch chaos activity
  watch kubectl get chaosengines,chaosresults -A
  ```

This namespace setup provides a production-ready foundation for running Litmus Chaos experiments with proper isolation, RBAC, and monitoring.
