/**
 * k6 Test Configuration
 * Shared configuration for all load tests
 */

// Environment-based configuration
export const config = {
  // Base URLs for services
  baseUrls: {
    dotconfig: __ENV.DOTCONFIG_URL || 'http://localhost:8080',
    grafana: __ENV.GRAFANA_URL || 'http://localhost:3000',
    influxdb: __ENV.INFLUXDB_URL || 'http://localhost:8086',
    redis: __ENV.REDIS_URL || 'localhost:6379',
    mongo: __ENV.MONGO_URL || 'localhost:27017',
  },

  // InfluxDB credentials
  influxdb: {
    org: __ENV.INFLUXDB_ORG || 'dotconfig',
    bucket: __ENV.INFLUXDB_BUCKET || 'telemetry',
    token: __ENV.INFLUXDB_TOKEN || 'my-super-secret-auth-token',
  },

  // Grafana credentials
  grafana: {
    username: __ENV.GRAFANA_USER || 'admin',
    password: __ENV.GRAFANA_PASSWORD || 'admin123',
  },

  // Test thresholds
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.01'],
    http_reqs: ['rate>10'],
  },
};

// Load test scenarios
export const scenarios = {
  // Smoke test - minimal load
  smoke: {
    executor: 'constant-vus',
    vus: 1,
    duration: '30s',
  },

  // Load test - normal traffic
  load: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '1m', target: 10 },
      { duration: '3m', target: 10 },
      { duration: '1m', target: 0 },
    ],
  },

  // Stress test - find breaking point
  stress: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '2m', target: 20 },
      { duration: '5m', target: 20 },
      { duration: '2m', target: 50 },
      { duration: '5m', target: 50 },
      { duration: '2m', target: 0 },
    ],
  },

  // Spike test - sudden traffic burst
  spike: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '10s', target: 100 },
      { duration: '1m', target: 100 },
      { duration: '10s', target: 0 },
    ],
  },

  // Soak test - sustained load
  soak: {
    executor: 'constant-vus',
    vus: 10,
    duration: '30m',
  },
};

// Helper to get scenario by name
export function getScenario(name) {
  return scenarios[name] || scenarios.smoke;
}
