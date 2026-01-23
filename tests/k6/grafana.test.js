/**
 * k6 Load Tests for Grafana
 *
 * Run: k6 run tests/k6/grafana.test.js
 * With options: k6 run --env SCENARIO=load tests/k6/grafana.test.js
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { config, getScenario } from './config.js';

// Custom metrics
const loginSuccess = new Rate('grafana_login_success');
const dashboardLoadTime = new Trend('grafana_dashboard_load_time');
const apiResponseTime = new Trend('grafana_api_response_time');

// Test configuration
const scenario = __ENV.SCENARIO || 'smoke';
export const options = {
  scenarios: {
    default: getScenario(scenario),
  },
  thresholds: {
    http_req_duration: ['p(95)<500'],
    http_req_failed: ['rate<0.05'],
    grafana_login_success: ['rate>0.95'],
  },
};

const BASE_URL = config.baseUrls.grafana;
const AUTH = {
  username: config.grafana.username,
  password: config.grafana.password,
};

export default function () {
  group('Health Check', () => {
    const health = http.get(`${BASE_URL}/api/health`);
    check(health, {
      'health status is 200': (r) => r.status === 200,
      'health response has database': (r) => r.json('database') === 'ok',
    });
  });

  group('Authentication', () => {
    // Login
    const loginRes = http.post(
      `${BASE_URL}/login`,
      JSON.stringify({
        user: AUTH.username,
        password: AUTH.password,
      }),
      {
        headers: { 'Content-Type': 'application/json' },
      }
    );

    const loginOk = check(loginRes, {
      'login status is 200': (r) => r.status === 200,
      'login returns message': (r) => r.json('message') === 'Logged in',
    });

    loginSuccess.add(loginOk);

    if (loginOk) {
      // Get session cookie for authenticated requests
      const cookies = loginRes.cookies;

      group('Dashboards', () => {
        // List dashboards
        const start = Date.now();
        const dashboards = http.get(`${BASE_URL}/api/search?type=dash-db`, {
          cookies: cookies,
        });
        dashboardLoadTime.add(Date.now() - start);

        check(dashboards, {
          'dashboards list status is 200': (r) => r.status === 200,
          'dashboards is array': (r) => Array.isArray(r.json()),
        });
      });

      group('Datasources', () => {
        const start = Date.now();
        const datasources = http.get(`${BASE_URL}/api/datasources`, {
          cookies: cookies,
        });
        apiResponseTime.add(Date.now() - start);

        check(datasources, {
          'datasources status is 200': (r) => r.status === 200,
          'datasources is array': (r) => Array.isArray(r.json()),
        });
      });

      group('Organizations', () => {
        const orgs = http.get(`${BASE_URL}/api/orgs`, {
          cookies: cookies,
        });

        check(orgs, {
          'orgs status is 200': (r) => r.status === 200,
        });
      });
    }
  });

  sleep(1);
}

export function handleSummary(data) {
  return {
    'tests/k6/results/grafana-summary.json': JSON.stringify(data, null, 2),
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
  };
}

function textSummary(data, opts) {
  const metrics = data.metrics;
  let output = '\n=== Grafana Load Test Summary ===\n\n';

  output += `Duration: ${data.state.testRunDurationMs}ms\n`;
  output += `VUs: ${data.metrics.vus?.values?.value || 0}\n\n`;

  if (metrics.http_reqs) {
    output += `Requests: ${metrics.http_reqs.values.count}\n`;
    output += `Request Rate: ${metrics.http_reqs.values.rate.toFixed(2)}/s\n`;
  }

  if (metrics.http_req_duration) {
    output += `\nResponse Times:\n`;
    output += `  avg: ${metrics.http_req_duration.values.avg.toFixed(2)}ms\n`;
    output += `  p95: ${metrics.http_req_duration.values['p(95)'].toFixed(2)}ms\n`;
    output += `  p99: ${metrics.http_req_duration.values['p(99)'].toFixed(2)}ms\n`;
  }

  if (metrics.grafana_login_success) {
    output += `\nLogin Success Rate: ${(metrics.grafana_login_success.values.rate * 100).toFixed(1)}%\n`;
  }

  return output;
}
