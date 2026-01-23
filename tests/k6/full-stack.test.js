/**
 * k6 Full Stack Load Test
 * Tests all services together to simulate real-world usage
 *
 * Run: k6 run tests/k6/full-stack.test.js
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { config } from './config.js';

// Custom metrics
const serviceAvailability = new Rate('service_availability');
const e2eLatency = new Trend('e2e_latency');
const errorCount = new Counter('errors');

// Test configuration
export const options = {
  scenarios: {
    // Simulate different user behaviors
    readers: {
      executor: 'constant-vus',
      vus: 5,
      duration: '2m',
      exec: 'readWorkload',
    },
    writers: {
      executor: 'constant-vus',
      vus: 2,
      duration: '2m',
      exec: 'writeWorkload',
      startTime: '10s',
    },
    monitors: {
      executor: 'constant-vus',
      vus: 1,
      duration: '2m',
      exec: 'monitorWorkload',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<1000'],
    http_req_failed: ['rate<0.05'],
    service_availability: ['rate>0.95'],
    errors: ['count<10'],
  },
};

const GRAFANA_URL = config.baseUrls.grafana;
const INFLUXDB_URL = config.baseUrls.influxdb;
const API_URL = config.baseUrls.dotconfig;

const influxHeaders = {
  Authorization: `Token ${config.influxdb.token}`,
  'Content-Type': 'text/plain',
};

// Read-heavy workload (dashboards, queries)
export function readWorkload() {
  const start = Date.now();

  group('Read Operations', () => {
    // Query Grafana dashboards
    const dashboards = http.get(`${GRAFANA_URL}/api/search?type=dash-db`, {
      auth: `${config.grafana.username}:${config.grafana.password}`,
    });

    const dashOk = check(dashboards, {
      'dashboards accessible': (r) => r.status === 200,
    });
    serviceAvailability.add(dashOk);

    // Query InfluxDB
    const query = `from(bucket: "${config.influxdb.bucket}") |> range(start: -5m) |> limit(n: 10)`;
    const queryRes = http.post(
      `${INFLUXDB_URL}/api/v2/query?org=${config.influxdb.org}`,
      JSON.stringify({ query, type: 'flux' }),
      {
        headers: {
          Authorization: `Token ${config.influxdb.token}`,
          'Content-Type': 'application/json',
          Accept: 'application/csv',
        },
      }
    );

    const queryOk = check(queryRes, {
      'influxdb query works': (r) => r.status === 200,
    });
    serviceAvailability.add(queryOk);

    if (!dashOk || !queryOk) {
      errorCount.add(1);
    }
  });

  e2eLatency.add(Date.now() - start);
  sleep(1);
}

// Write-heavy workload (metrics ingestion)
export function writeWorkload() {
  const start = Date.now();

  group('Write Operations', () => {
    // Write metrics to InfluxDB
    const timestamp = Date.now() * 1000000;
    const metrics = [];

    for (let i = 0; i < 5; i++) {
      metrics.push(
        `app_metrics,service=api,endpoint=/health response_time=${Math.random() * 100},status_code=200 ${timestamp + i}`
      );
      metrics.push(
        `app_metrics,service=api,endpoint=/ready response_time=${Math.random() * 50},status_code=200 ${timestamp + i + 100}`
      );
    }

    const writeRes = http.post(
      `${INFLUXDB_URL}/api/v2/write?org=${config.influxdb.org}&bucket=${config.influxdb.bucket}&precision=ns`,
      metrics.join('\n'),
      { headers: influxHeaders }
    );

    const writeOk = check(writeRes, {
      'influxdb write works': (r) => r.status === 204,
    });
    serviceAvailability.add(writeOk);

    if (!writeOk) {
      errorCount.add(1);
    }
  });

  e2eLatency.add(Date.now() - start);
  sleep(2);
}

// Health monitoring workload
export function monitorWorkload() {
  const start = Date.now();

  group('Health Monitoring', () => {
    const services = [
      { name: 'grafana', url: `${GRAFANA_URL}/api/health` },
      { name: 'influxdb', url: `${INFLUXDB_URL}/health` },
      { name: 'api', url: `${API_URL}/health` },
    ];

    for (const svc of services) {
      const res = http.get(svc.url, { timeout: '5s' });
      const ok = check(res, {
        [`${svc.name} healthy`]: (r) => r.status === 200,
      });
      serviceAvailability.add(ok);

      if (!ok) {
        console.warn(`${svc.name} health check failed: ${res.status}`);
        errorCount.add(1);
      }
    }
  });

  e2eLatency.add(Date.now() - start);
  sleep(5);
}

export function handleSummary(data) {
  const summary = {
    timestamp: new Date().toISOString(),
    duration: data.state.testRunDurationMs,
    metrics: {
      requests: data.metrics.http_reqs?.values?.count || 0,
      requestRate: data.metrics.http_reqs?.values?.rate || 0,
      avgLatency: data.metrics.http_req_duration?.values?.avg || 0,
      p95Latency: data.metrics.http_req_duration?.values['p(95)'] || 0,
      errorRate: data.metrics.http_req_failed?.values?.rate || 0,
      availability: data.metrics.service_availability?.values?.rate || 0,
    },
  };

  return {
    'tests/k6/results/full-stack-summary.json': JSON.stringify(summary, null, 2),
    stdout: `
=== Full Stack Load Test Summary ===

Duration: ${(summary.duration / 1000).toFixed(1)}s
Total Requests: ${summary.metrics.requests}
Request Rate: ${summary.metrics.requestRate.toFixed(2)}/s

Latency:
  Average: ${summary.metrics.avgLatency.toFixed(2)}ms
  P95: ${summary.metrics.p95Latency.toFixed(2)}ms

Service Availability: ${(summary.metrics.availability * 100).toFixed(1)}%
Error Rate: ${(summary.metrics.errorRate * 100).toFixed(2)}%
`,
  };
}
