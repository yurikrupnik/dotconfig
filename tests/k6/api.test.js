/**
 * k6 Load Tests for dotconfig API
 *
 * Run: k6 run tests/k6/api.test.js
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { config, getScenario } from './config.js';

// Custom metrics
const apiSuccess = new Rate('api_success');
const apiLatency = new Trend('api_latency');

// Test configuration
const scenario = __ENV.SCENARIO || 'smoke';
export const options = {
  scenarios: {
    default: getScenario(scenario),
  },
  thresholds: {
    http_req_duration: ['p(95)<300', 'p(99)<500'],
    http_req_failed: ['rate<0.01'],
    api_success: ['rate>0.99'],
  },
};

const BASE_URL = config.baseUrls.dotconfig;

export default function () {
  group('Health Endpoints', () => {
    // Health check
    const start = Date.now();
    const health = http.get(`${BASE_URL}/health`);
    apiLatency.add(Date.now() - start);

    const healthOk = check(health, {
      'health status is 200': (r) => r.status === 200,
    });
    apiSuccess.add(healthOk);

    // Ready check
    const ready = http.get(`${BASE_URL}/ready`);
    check(ready, {
      'ready status is 200': (r) => r.status === 200,
    });

    // Live check
    const live = http.get(`${BASE_URL}/live`);
    check(live, {
      'live status is 200': (r) => r.status === 200,
    });
  });

  group('Metrics Endpoint', () => {
    const metrics = http.get(`${BASE_URL}/metrics`);
    check(metrics, {
      'metrics status is 200': (r) => r.status === 200,
      'metrics has content': (r) => r.body.length > 0,
    });
  });

  group('API Endpoints', () => {
    // Version endpoint
    const version = http.get(`${BASE_URL}/api/version`);
    check(version, {
      'version status is 200 or 404': (r) => r.status === 200 || r.status === 404,
    });

    // Status endpoint
    const status = http.get(`${BASE_URL}/api/status`);
    check(status, {
      'status returns valid response': (r) => r.status < 500,
    });
  });

  sleep(0.5);
}

export function handleSummary(data) {
  return {
    'tests/k6/results/api-summary.json': JSON.stringify(data, null, 2),
  };
}
