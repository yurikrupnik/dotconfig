# Example Manifests

Complete example configurations for all platform integrations.

## Directory Structure

```
k8s-manifests/examples/
├── dapr-flagsmith/          # Dapr + Flagsmith integration
│   ├── deployment.yaml      # Application with Dapr sidecar
│   └── dapr-components.yaml # Dapr components (pubsub, state, config)
├── canary-deployment/       # Flagger + KEDA integration
│   ├── canary.yaml          # Flagger canary configuration
│   ├── scaledobject.yaml    # KEDA autoscaling
│   └── networking.yaml      # Istio gateway and virtual service
├── chaos/                   # Chaos Mesh experiments
│   └── experiments.yaml     # Various chaos experiments
└── crossplane/              # Crossplane resources
    └── resources.yaml       # Cloud resource definitions
```

## Usage

### Quick Deploy All Examples

```bash
# Deploy all examples
kubectl apply -f k8s-manifests/examples/

# Verify deployments
kubectl get all -n default
kubectl get canaries
kubectl get scaledobjects
kubectl get podchaos
kubectl get managed
```

### Individual Examples

#### 1. Dapr + Flagsmith

```bash
# Deploy Dapr components
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/dapr-components.yaml

# Deploy application
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/deployment.yaml

# Verify
kubectl get components.dapr.io
kubectl get pods -l app=dotconfig

# Test Dapr invocation
dapr invoke --app-id dotconfig --method health
```

#### 2. Flagger + KEDA

```bash
# Deploy canary configuration
kubectl apply -f k8s-manifests/examples/canary-deployment/canary.yaml

# Deploy autoscaling
kubectl apply -f k8s-manifests/examples/canary-deployment/scaledobject.yaml

# Deploy networking
kubectl apply -f k8s-manifests/examples/canary-deployment/networking.yaml

# Trigger canary
kubectl set image deployment/dotconfig dotconfig=yurikrupnik/dotconfig:v2.0

# Monitor
kubectl get canary dotconfig -w
kubectl describe canary dotconfig
```

#### 3. Chaos Mesh

```bash
# Deploy chaos experiments
kubectl apply -f k8s-manifests/examples/chaos/experiments.yaml

# List experiments
kubectl get podchaos, networkchaos, iochaos, stresschaos

# Monitor application resilience
kubectl logs -f deployment/dotconfig

# Pause experiments
kubectl patch podchaos dotconfig-pod-failure -p '{"spec":{"pause":true}}'

# Delete experiments
kubectl delete podchaos dotconfig-pod-failure
```

#### 4. Crossplane

```bash
# Setup AWS credentials
kubectl create secret generic aws-creds \
  -n crossplane-system \
  --from-file=credentials=./aws-credentials.txt

# Deploy Crossplane resources
kubectl apply -f k8s-manifests/examples/crossplane/resources.yaml

# Monitor resource creation
kubectl get managed -w

# Check S3 bucket
kubectl get bucket
kubectl describe bucket dotconfig-backup-bucket

# Check RDS instance
kubectl get instance.rds.aws.crossplane.io
kubectl describe instance dotconfig-db
```

## Customization

### Modify for Your Application

1. **Update image name** in deployment.yaml:
   ```yaml
   image: your-registry/your-app:v1.0
   ```

2. **Update Dapr app ID**:
   ```yaml
   annotations:
     dapr.io/app-id: "your-app-id"
   ```

3. **Update feature flag keys**:
   ```yaml
   envFrom:
   - secretRef:
       name: your-flagsmith-secret
   ```

4. **Update autoscaling thresholds**:
   ```yaml
   triggers:
   - type: prometheus
     metadata:
       threshold: "500"  # Adjust based on your metrics
   ```

### Add Custom Components

#### Dapr Component (Pub/Sub)

```yaml
apiVersion: dapr.io/v1alpha1
kind: Component
metadata:
  name: custom-pubsub
spec:
  type: pubsub.kafka
  version: v1
  metadata:
  - name: brokers
    value: "kafka:9092"
  - name: consumerGroup
    value: "my-group"
```

#### KEDA Trigger

```yaml
triggers:
- type: kafka
  metadata:
    bootstrapServers: kafka:9092
    consumerGroup: my-group
    topic: events
    lagThreshold: "100"
```

#### Chaos Experiment

```yaml
apiVersion: chaos-mesh.org/v1alpha1
kind: HttpChaos
metadata:
  name: http-chaos
spec:
  mode: one
  selector:
    labelSelectors:
      app: dotconfig
  target: Request
  port: 8080
  path: /api
  method: GET
  delay: "200ms"
  duration: "10s"
```

## Monitoring Examples

### Application Metrics

```bash
# Check application metrics
kubectl exec -it deployment/dotconfig -- curl http://localhost:8080/metrics

# Check Dapr metrics
kubectl exec -it deployment/dotconfig -c daprd -- curl http://localhost:9090/metrics
```

### Flagger Metrics

```bash
# Check canary metrics
kubectl get canary dotconfig -o jsonpath='{.status.canaryProgress}'

# View Flagger events
kubectl get events --field-selector involvedObject.kind=Canary
```

### KEDA Metrics

```bash
# Check HPA metrics
kubectl get hpa
kubectl describe hpa dotconfig

# Check ScaledObject metrics
kubectl get scaledobject dotconfig -o yaml
```

### Chaos Metrics

```bash
# Check chaos events
kubectl get events --field-selector involvedObject.kind=PodChaos

# View chaos dashboard
open http://localhost:2333
```

## Integration Examples

### End-to-End Flow

```bash
# 1. Deploy all components
kubectl apply -f k8s-manifests/examples/dapr-flagsmith/
kubectl apply -f k8s-manifests/examples/canary-deployment/
kubectl apply -f k8s-manifests/examples/chaos/

# 2. Trigger canary deployment
kubectl set image deployment/dotconfig dotconfig=yurikrupnik/dotconfig:v2.0

# 3. Simulate chaos
kubectl apply -f k8s-manifests/examples/chaos/pod-failure.yaml

# 4. Monitor observability
kubectl logs -f deployment/dotconfig
kubectl get canary dotconfig -w
kubectl get hpa -w

# 5. Verify feature flags
curl http://dotconfig/feature-flags
```

### Feature Flag Toggle

```bash
# Enable feature via Flagsmith UI or API
curl -X POST http://flagsmith-api:8000/api/v1/features/ \
  -H "X-Environment-Key: <your-key>" \
  -d '{"name":"new_feature","is_enabled":true}'

# Application responds to flag change
curl http://dotconfig/api/feature/new_feature
```

### Autoscaling Test

```bash
# Generate load
hey -n 10000 -c 100 http://dotconfig/

# Watch HPA scale up
watch kubectl get hpa

# Stop load
watch kubectl get pods -l app=dotconfig
```

### Chaos Resilience Test

```bash
# Apply pod failure
kubectl apply -f chaos/pod-failure.yaml

# Monitor recovery
kubectl get pods -w

# Check service availability
watch curl -s http://dotconfig/health
```

## Cleanup

```bash
# Remove all examples
kubectl delete -f k8s-manifests/examples/

# Remove specific components
kubectl delete canary dotconfig
kubectl delete scaledobject dotconfig-scaledobject
kubectl delete podchaos dotconfig-pod-failure

# Remove crossplane resources
kubectl delete bucket dotconfig-backup-bucket
kubectl delete instance dotconfig-db
```

## Next Steps

- Customize manifests for your application
- Add your own chaos experiments
- Configure custom metrics for autoscaling
- Set up Crossplane providers for your cloud provider
- Implement advanced Flagger A/B testing

See [PLATFORM.md](PLATFORM.md) for full platform documentation.
