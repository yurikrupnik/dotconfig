# Chaos Experiments

Complete chaos engineering scenarios for testing application resilience.

## Overview

These manifests provide various chaos experiments to test:
- **Pod Chaos**: Pod failures and container kills
- **Network Chaos**: Latency, packet loss, partitions, DNS issues
- **IO Chaos**: Disk latency, faults, and limits
- **Stress Chaos**: CPU, memory, and disk stress
- **Advanced Chaos**: Kernel failures, time shifts, HTTP faults

## Quick Start

```bash
# Apply all chaos experiments
kubectl apply -f k8s-manifests/examples/chaos/

# List active experiments
kubectl get podchaos, networkchaos, iochaos, stresschaos, kernelchaos, httpchaos, timechaos, dnschaos

# Monitor chaos events
kubectl get events --field-selector involvedObject.kind=PodChaos
```

## Experiment Types

### 1. Pod Chaos

**Pod Kill** - Randomly kills pods
```bash
kubectl apply -f k8s-manifests/examples/chaos/pod-chaos.yaml
```

Tests:
- Application recovery
- Pod recreation
- State management

### 2. Network Chaos

**Network Partition** - Simulates network outage
```bash
kubectl apply -f k8s-manifests/examples/chaos/network-chaos.yaml
```

Tests:
- Service discovery
- Circuit breakers
- Retry logic

**Network Latency** - Adds network delays
```bash
kubectl get networkchaos dotconfig-network-delay
```

Tests:
- Timeout handling
- Performance degradation
- User experience impact

### 3. IO Chaos

**Disk Latency** - Simulates slow disk
```bash
kubectl apply -f k8s-manifests/examples/chaos/io-chaos.yaml
```

Tests:
- Logging performance
- File operations
- Application responsiveness

### 4. Stress Chaos

**CPU Stress** - High CPU usage
```bash
kubectl apply -f k8s-manifests/examples/chaos/stress-chaos.yaml
```

Tests:
- Autoscaling response
- Resource limits
- Performance under load

### 5. Advanced Chaos

**Time Shift** - Changes system time
```bash
kubectl apply -f k8s-manifests/examples/chaos/advanced-chaos.yaml
```

Tests:
- Time-based logic
- Token expiration
- Scheduled tasks

## Chaos Workflow

Create a workflow with multiple experiments:

```yaml
apiVersion: chaos-mesh.org/v1alpha1
kind: Workflow
metadata:
  name: resilience-test
  namespace: default
spec:
  entry: entry
  templates:
    - name: entry
      templateType: Serial
      deadline: 30m
      children:
        - pod-kill
        - network-delay
        - stress-cpu
    
    - name: pod-kill
      templateType: PodChaos
      deadline: 5m
      podChaos:
        action: pod-kill
        mode: one
        selector:
          namespaces:
            - default
          labelSelectors:
            app: dotconfig
    
    - name: network-delay
      templateType: NetworkChaos
      deadline: 5m
      networkChaos:
        action: delay
        mode: one
        selector:
          namespaces:
            - default
          labelSelectors:
            app: dotconfig
        delay:
          latency: "200ms"
    
    - name: stress-cpu
      templateType: StressChaos
      deadline: 5m
      stressChaos:
        mode: one
        selector:
          namespaces:
            - default
          labelSelectors:
            app: dotconfig
        stressors:
          cpu:
            workers: 2
            load: 80
```

## Monitoring Chaos Experiments

### Dashboard

```bash
# Access Chaos Mesh dashboard
kubectl port-forward -n chaos-mesh svc/chaos-dashboard 2333:2333
open http://localhost:2333
```

### Logs

```bash
# Monitor application logs during chaos
kubectl logs -f deployment/dotconfig

# Check chaos events
kubectl get events --field-selector involvedObject.kind=PodChaos --watch
```

### Metrics

```bash
# View chaos metrics in Prometheus
curl http://localhost:9090/api/v1/query?query=chaos_experiments_active
```

## Safety First

### Pausing Experiments

```bash
# Pause all chaos
kubectl patch podchaos dotconfig-pod-failure -p '{"spec":{"pause":true}}'

# Pause specific experiment
kubectl annotate chaos dotconfig-pod-failure chaos-mesh.org/pause=true
```

### Cleaning Up

```bash
# Remove all chaos
kubectl delete -f k8s-manifests/examples/chaos/

# Remove specific experiment
kubectl delete podchaos dotconfig-pod-failure
kubectl delete networkchaos dotconfig-network-delay
```

## Best Practices

1. **Start Small**: Begin with mild experiments (10% failure rate)
2. **Monitor Closely**: Watch logs, metrics, and alerts
3. **Gradual Increase**: Increase intensity over time
4. **Test in Dev**: Always test in dev/staging first
5. **Document Results**: Record resilience test outcomes

## Emergency Recovery

```bash
# Kill all chaos immediately
kubectl delete podchaos, networkchaos, iochaos, stresschaos, kernelchaos, httpchaos, timechaos, dnschaos -A

# Scale up pods
kubectl scale deployment dotconfig --replicas=3

# Check pod status
kubectl get pods -l app=dotconfig

# Restart pods if needed
kubectl rollout restart deployment/dotconfig
```

## Integration with Flagger

Chaos experiments can be combined with canary deployments:

```bash
# 1. Trigger canary
kubectl set image deployment/dotconfig dotconfig=v2.0

# 2. Apply chaos during canary
kubectl apply -f k8s-manifests/examples/chaos/network-chaos.yaml

# 3. Monitor canary progress
kubectl get canary dotconfig -w

# 4. If canary fails, chaos helped identify the issue!
```

## Troubleshooting

### Pods Not Recovering

```bash
# Check pod events
kubectl describe pod <pod-name>

# Check if chaos is still running
kubectl get podchaos

# Kill chaos
kubectl delete podchaos --all
```

### Application Completely Down

```bash
# Check deployment status
kubectl get deployment dotconfig

# Restart deployment
kubectl rollout restart deployment dotconfig

# Check if chaos killed all pods
kubectl get podchaos -o yaml | grep "percentage"
```

### Network Still Partitioned

```bash
# Delete network chaos
kubectl delete networkchaos --all

# Restart pods to reconnect
kubectl rollout restart deployment dotconfig

# Verify network
kubectl exec -it deployment/dotconfig -- ping redis
```

## Learn More

- [Chaos Mesh Documentation](https://chaos-mesh.org/docs/)
- [Principles of Chaos Engineering](https://principlesofchaos.org/)
- [Chaos Engineering Best Practices](https://www.gremlin.com/community/tutorials/chaos-engineering-best-practices/)
