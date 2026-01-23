/**
 * k6 Load Tests for InfluxDB
 *
 * Run: k6 run tests/k6/influxdb.test.js
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { config, getScenario } from './config.js';

// Custom metrics
const writeSuccess = new Rate('influxdb_write_success');
const querySuccess = new Rate('influxdb_query_success');
const writeLatency = new Trend('influxdb_write_latency');
const queryLatency = new Trend('influxdb_query_latency');
const pointsWritten = new Counter('influxdb_points_written');

// Test configuration
const scenario = __ENV.SCENARIO || 'smoke';
export const options = {
  scenarios: {
    default: getScenario(scenario),
  },
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    influxdb_write_success: ['rate>0.99'],
    influxdb_query_success: ['rate>0.99'],
  },
};

const BASE_URL = config.baseUrls.influxdb;
const ORG = config.influxdb.org;
const BUCKET = config.influxdb.bucket;
const TOKEN = config.influxdb.token;

const headers = {
  Authorization: `Token ${TOKEN}`,
  'Content-Type': 'text/plain; charset=utf-8',
};

const jsonHeaders = {
  Authorization: `Token ${TOKEN}`,
  'Content-Type': 'application/json',
};

export default function () {
  const testId = `test-${__VU}-${__ITER}`;

  group('Health Check', () => {
    const health = http.get(`${BASE_URL}/health`);
    check(health, {
      'health status is 200': (r) => r.status === 200,
      'health status is pass': (r) => r.json('status') === 'pass',
    });
  });

  group('Ping', () => {
    const ping = http.get(`${BASE_URL}/ping`);
    check(ping, {
      'ping status is 204': (r) => r.status === 204,
    });
  });

  group('Write Data', () => {
    // Generate line protocol data
    const timestamp = Date.now() * 1000000; // nanoseconds
    const lines = [];

    // Write multiple metrics
    for (let i = 0; i < 10; i++) {
      const cpu = Math.random() * 100;
      const mem = Math.random() * 100;
      lines.push(
        `system,host=server${i},region=us-east cpu=${cpu.toFixed(2)},memory=${mem.toFixed(2)} ${timestamp + i}`
      );
    }

    const data = lines.join('\n');
    const start = Date.now();

    const writeRes = http.post(
      `${BASE_URL}/api/v2/write?org=${ORG}&bucket=${BUCKET}&precision=ns`,
      data,
      { headers }
    );

    writeLatency.add(Date.now() - start);

    const writeOk = check(writeRes, {
      'write status is 204': (r) => r.status === 204,
    });

    writeSuccess.add(writeOk);
    if (writeOk) {
      pointsWritten.add(10);
    }
  });

  group('Query Data', () => {
    const fluxQuery = `
      from(bucket: "${BUCKET}")
        |> range(start: -1h)
        |> filter(fn: (r) => r._measurement == "system")
        |> limit(n: 100)
    `;

    const start = Date.now();

    const queryRes = http.post(
      `${BASE_URL}/api/v2/query?org=${ORG}`,
      JSON.stringify({ query: fluxQuery, type: 'flux' }),
      {
        headers: {
          ...jsonHeaders,
          Accept: 'application/csv',
        },
      }
    );

    queryLatency.add(Date.now() - start);

    const queryOk = check(queryRes, {
      'query status is 200': (r) => r.status === 200,
      'query has data': (r) => r.body.length > 0,
    });

    querySuccess.add(queryOk);
  });

  group('Buckets API', () => {
    const buckets = http.get(`${BASE_URL}/api/v2/buckets?org=${ORG}`, {
      headers: jsonHeaders,
    });

    check(buckets, {
      'buckets status is 200': (r) => r.status === 200,
      'buckets has data': (r) => r.json('buckets') !== undefined,
    });
  });

  sleep(0.5);
}

export function handleSummary(data) {
  return {
    'tests/k6/results/influxdb-summary.json': JSON.stringify(data, null, 2),
  };
}
