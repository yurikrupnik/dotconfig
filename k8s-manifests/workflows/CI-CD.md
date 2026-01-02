# CI/CD Workflows

Complete CI/CD pipeline configurations for automated testing, building, and deployment with chaos testing.

## Overview

These workflows provide:
- **GitHub Actions**: Automated CI/CD with chaos testing
- **Tekton Pipelines**: Kubernetes-native pipeline
- **Argo Workflows**: Workflow automation

## GitHub Actions Pipeline

### Pipeline Stages

1. **Test**
   - Rust format check
   - Clippy linter
   - Unit tests with coverage
   - Security scanning

2. **Build**
   - Multi-arch Docker image build
   - Container registry push
   - Vulnerability scanning with Trivy

3. **Deploy to Staging**
   - Kubernetes deployment
   - Smoke tests
   - Chaos experiments

4. **Canary to Production**
   - Flagger canary deployment
   - Automated traffic shift
   - Health checks

5. **Chaos Testing**
   - Pod failure experiments
   - Resilience validation

### Usage

```bash
# Pipeline runs automatically on push to main/develop
# Manual trigger available in GitHub Actions UI

# View pipeline status
gh run list

# View specific run
gh run view <run-id>

# Trigger manual workflow
gh workflow run ci-cd.yml
```

## Tekton Pipeline

### Components

- **Pipeline**: dotconfig-pipeline
- **Triggers**: GitHub webhook integration
- **Tasks**: build, deploy, test, chaos

### Installation

```bash
# Apply Tekton pipeline
kubectl apply -f .github/tekton-pipeline.yaml

# Create GitHub webhook pointing to:
# https://<tekton-elb>/hooks/<eventlistener>
```

### Usage

```bash
# Trigger pipeline manually
kubectl create -f trigger-manual.yaml

# Monitor pipeline
tkn pipelinerun list
tkn pipelinerun logs <run-name>

# View pipeline graph
tkn pipeline describe dotconfig-pipeline
```

## Argo Workflows

### Workflow Steps

1. Build image with Kaniko
2. Deploy to staging
3. Run smoke tests
4. Apply chaos experiments
5. Deploy to production
6. Monitor canary

### Usage

```bash
# Submit workflow
kubectl apply -f .github/workflows/argo-workflow.yaml

# Monitor workflow
argo list
argo get <workflow-name>

# View workflow visualization
argo dashboard
```

## Chaos Testing in CI/CD

### Staging Chaos

```yaml
# Apply chaos after deployment
kubectl apply -f k8s-manifests/examples/chaos/pod-chaos.yaml

# Monitor resilience
sleep 60
kubectl get pods -l app=dotconfig

# Verify recovery
kubectl wait --for=condition=available --timeout=120s deployment/dotconfig

# Clean up
kubectl delete -f k8s-manifests/examples/chaos/pod-chaos.yaml
```

### Production Canary + Chaos

```yaml
# Trigger canary
kubectl set image deployment/dotconfig dotconfig=v2.0

# During canary analysis, apply mild chaos
kubectl apply -f k8s-manifests/examples/chaos/network-delay.yaml

# Monitor Flagger + Chaos
kubectl get canary dotconfig -w

# Canary succeeds if it survives chaos!
```

## Secrets Required

### GitHub Actions

```bash
# Set up secrets
gh secret set KUBE_CONFIG_STAGING < staging-kubeconfig.txt
gh secret set KUBE_CONFIG_PROD < prod-kubeconfig.txt
gh secret set CODECOV_TOKEN < your-codecov-token>
gh secret set SLACK_WEBHOOK < your-slack-webhook-url>
```

### Tekton

```bash
# Create secret for registry
kubectl create secret docker-registry regcred \
  --docker-server=ghcr.io \
  --docker-username=yurikrupnik \
  --docker-password=<gh-token>

# Create kubeconfig secret
kubectl create secret generic kubeconfig \
  --from-file=config=<kubeconfig-file>
```

## Monitoring Pipelines

### GitHub Actions

```bash
# View recent runs
gh run list --limit 10

# Watch specific run
gh run watch <run-id>

# Download artifacts
gh run download <run-id>
```

### Tekton

```bash
# List pipelineruns
kubectl get pipelineruns

# View logs
tkn pipelinerun logs <run-name>

# Describe pipeline
kubectl describe pipelinerun <run-name>
```

### Argo

```bash
# List workflows
argo list

# Get workflow details
argo get <workflow-name>

# View workflow logs
argo logs <workflow-name>
```

## Best Practices

### Testing

- **Unit Tests**: Run on every commit
- **Integration Tests**: Run on PR
- **Chaos Tests**: Run on staging
- **Canary Tests**: Run on production

### Deployment

- **Staging**: Always deploy first
- **Chaos Test**: Validate resilience
- **Canary**: Gradual rollout
- **Monitor**: Watch metrics and logs

### Rollback

```bash
# GitHub Actions: Failed workflow triggers rollback
# Automatic rollback via Flagger on canary failure

# Manual rollback
kubectl rollout undo deployment/dotconfig -n production

# Check rollout status
kubectl rollout status deployment/dotconfig -n production
```

## Integration with Platform Components

### Flagger

```bash
# Canary deployment monitored by Flagger
kubectl set image deployment/dotconfig dotconfig=v2.0

# Flagger automatically:
# - Creates canary pods
# - Routes traffic gradually
# - Runs metrics checks
# - Promotes or rolls back
```

### KEDA

```bash
# HPA scales based on load
# CI/CD generates load to test autoscaling

# Generate load
hey -n 10000 -c 100 http://dotconfig/

# Watch scaling
watch kubectl get hpa
```

### Chaos Mesh

```bash
# CI/CD applies chaos experiments
kubectl apply -f k8s-manifests/examples/chaos/

# Monitor resilience
kubectl logs -f deployment/dotconfig
```

### Observability

```bash
# All pipeline events sent to Prometheus
# Grafana dashboards show deployment health
# Jaeger traces show request flow during deployment
```

## Troubleshooting

### Pipeline Failing

```bash
# Check logs
gh run view <run-id> --log

# Check cluster status
kubectl get pods,deployments,services

# Check chaos status
kubectl get podchaos, networkchaos
```

### Canary Stuck

```bash
# Check Flagger logs
kubectl logs -n istio-system deployment/flagger

# Check canary status
kubectl get canary dotconfig -o yaml

# Describe canary
kubectl describe canary dotconfig
```

### Chaos Experiments Not Cleaning Up

```bash
# Force delete chaos
kubectl delete podchaos, networkchaos --all

# Restart pods
kubectl rollout restart deployment/dotconfig
```

## Metrics & Alerts

### CI/CD Metrics

- Pipeline duration
- Success rate
- Failure points
- Chaos test results

### Prometheus Alerts

```yaml
# Pipeline failure
- alert: CIPipelineFailed
  expr: github_actions_run_status{status="failed"} > 0

# Canary failure
- alert: CanaryDeploymentFailed
  expr: flagger_canary_status{"status":"failed"} > 0

# Chaos failure
- alert: ChaosTestFailed
  expr: chaos_test_result == 0
```

## Next Steps

- Customize pipelines for your application
- Add more chaos experiments
- Implement automated rollback
- Set up notifications (Slack, Discord, Email)
- Add performance tests
- Implement A/B testing with Flagger

## Resources

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Tekton Docs](https://tekton.dev/docs/)
- [Argo Workflows Docs](https://argoproj.github.io/argo-workflows/)
- [Flagger Progressive Delivery](https://flagger.app/)
- [Chaos Mesh CI/CD](https://chaos-mesh.org/docs/simulate-chaos-with-chaos-mesh/)
