# Local Development Environment

Complete local development setup for the dotconfig platform with Docker Compose.

## Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- 8GB+ RAM
- 20GB+ free disk space

## Quick Start

```bash
# Clone repository
git clone https://github.com/yurikrupnik/dotconfig.git
cd dotconfig

# Start all services
docker compose up -d

# Verify all services are running
docker compose ps

# View logs
docker compose logs -f
```

## Services

| Service | Port | URL | Purpose |
|---------|------|-----|---------|
| **dotconfig-app** | 8080 | http://localhost:8080 | Main application |
| **dapr-sidecar** | 3500, 50001 | - | Dapr sidecar |
| **nats** | 4222, 8222 | http://localhost:8222 | Message queue |
| **redis** | 6379 | - | State store |
| **flagsmith-api** | 8000 | http://localhost:8000 | Feature flag API |
| **flagsmith-frontend** | 3000 | http://localhost:3000 | Feature flag UI |
| **prometheus** | 9090 | http://localhost:9090 | Metrics |
| **grafana** | 3001 | http://localhost:3001 | Dashboards |
| **jaeger** | 16686 | http://localhost:16686 | Tracing |
| **chaos-dashboard** | 2333 | http://localhost:2333 | Chaos experiments |
| **loki** | 3100 | - | Log aggregation |
| **promtail** | - | - | Log collection |

## Usage

### Development Workflow

```bash
# Start services
docker compose up -d

# Rebuild application after changes
docker compose up -d --build dotconfig-app

# View application logs
docker compose logs -f dotconfig-app

# Enter application container
docker compose exec dotconfig-app sh

# Stop all services
docker compose down

# Stop and remove volumes
docker compose down -v
```

### Feature Flags with Flagsmith

1. Access Flagsmith UI: http://localhost:3000
2. Create a new project
3. Create feature flags
4. Get environment key
5. Update `FLAGSMITH_ENV_KEY` in docker-compose.yml
6. Restart: `docker compose up -d dotconfig-app`

### Testing with NATS

```bash
# Publish a message
docker compose exec nats nats pub "dotconfig.events" "hello world"

# Subscribe to messages
docker compose exec nats nats sub "dotconfig.events"
```

### Monitoring

```bash
# Check metrics in Prometheus
open http://localhost:9090

# View Grafana dashboards
open http://localhost:3001 (admin/admin)

# View traces in Jaeger
open http://localhost:16686

# View logs in Loki
docker compose logs loki
```

### Chaos Experiments

```bash
# Access Chaos Dashboard
open http://localhost:2333

# Create a pod failure experiment
kubectl apply -f k8s-manifests/examples/chaos/pod-failure.yaml
```

## Configuration

### Environment Variables

Edit `docker-compose.yml` to customize:

```yaml
environment:
  - RUST_LOG=info
  - FLAGSMITH_API_URL=http://flagsmith-frontend:3000
  - FLAGSMITH_ENV_KEY=local
```

### Resource Limits

```yaml
deploy:
  resources:
    limits:
      cpus: '0.50'
      memory: 512M
    reservations:
      cpus: '0.25'
      memory: 256M
```

## Troubleshooting

### Services Not Starting

```bash
# Check logs
docker compose logs

# Check resource usage
docker stats

# Restart specific service
docker compose restart <service-name>
```

### Port Conflicts

Edit `docker-compose.yml` to change port mappings:

```yaml
ports:
  - "9091:9090"  # Change Prometheus to 9091
```

### Database Issues

```bash
# Reset PostgreSQL
docker compose down
docker volume rm dotconfig_postgres-flagsmith-data
docker compose up -d postgres-flagsmith
```

## Development Tips

### Hot Reload

```bash
# Watch for changes and rebuild
docker compose watch

# Or use specific watch targets
docker compose watch dotconfig-app
```

### Testing

```bash
# Run tests in container
docker compose exec dotconfig-app cargo test

# Test feature flags
curl http://localhost:8080/feature-flags

# Test health endpoint
curl http://localhost:8080/health

# Test metrics endpoint
curl http://localhost:8080/metrics
```

### Debugging

```bash
# Enable debug logging
docker compose up -d -e RUST_LOG=debug dotconfig-app

# Attach debugger
docker compose exec dotconfig-app rust-gdb target/debug/dotconfig

# Check container networking
docker compose exec dotconfig-app ping nats
docker compose exec dotconfig-app curl http://flagsmith-api:8000
```

## Cleanup

```bash
# Stop and remove containers
docker compose down

# Remove volumes (WARNING: deletes data)
docker compose down -v

# Remove images
docker compose down --rmi all

# Full cleanup
docker compose down -v --rmi all --remove-orphans
```

## Next Steps

- See [PLATFORM.md](PLATFORM.md) for full platform documentation
- Deploy to Kubernetes: `nu scripts/nu/platform/stack.nu install-all`
- Configure observability: `config/prometheus/prometheus.yml`
- Create chaos experiments: `k8s-manifests/examples/chaos/`
