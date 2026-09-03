// devkit fleet — self-hosted fleet hub: agent ingest, agentless probes, history.
//
// Zero dependencies; runs on Bun. Machines that can run Bun push snapshots via
// fleet/agent.ts (POST /api/report). Devices that cannot host an agent —
// phones, tablets, printers, IoT — are probed by the hub itself (TCP connect
// when a port is configured, ICMP ping otherwise). History lives in a local
// SQLite file; the responsive UI in ui.html is served from /.
//
// Trust model: LAN tool. If FLEET_TOKEN is set, reports must present it as a
// bearer token so nothing on the network can poison the data; reads are open.
// Do not expose beyond a trusted network.

import { Database } from "bun:sqlite";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";

const PORT = Number(Bun.env.FLEET_PORT ?? 9300);
const TOKEN = Bun.env.FLEET_TOKEN ?? "";
const DB_PATH = Bun.env.FLEET_DB ?? `${process.env.HOME}/.local/share/devkit/fleet.db`;
const RETENTION_HOURS = Number(Bun.env.FLEET_RETENTION_HOURS ?? 48);
const PROBE_INTERVAL = Math.max(5, Number(Bun.env.FLEET_PROBE_INTERVAL ?? 30));
const STALE_AFTER = Number(Bun.env.FLEET_STALE_AFTER ?? 60); // s without report -> stale
const OFFLINE_AFTER = Number(Bun.env.FLEET_OFFLINE_AFTER ?? 300); // s without report -> offline
const SPARK_POINTS = 60; // per-machine sparkline resolution (last hour)

type ProbeTarget = { name: string; host: string; port?: number };
const PROBES: ProbeTarget[] = JSON.parse(Bun.env.FLEET_PROBES ?? "[]");

// --- Storage -----------------------------------------------------------------

mkdirSync(dirname(DB_PATH), { recursive: true });
const db = new Database(DB_PATH, { create: true });
db.exec(`
  PRAGMA journal_mode = WAL;
  CREATE TABLE IF NOT EXISTS machines (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,          -- 'agent' | 'probe'
    hostname   TEXT NOT NULL,
    platform   TEXT,
    arch       TEXT,
    cpus       INTEGER,
    mem_total  INTEGER,
    first_seen INTEGER NOT NULL,       -- unix seconds
    last_seen  INTEGER NOT NULL
  );
  CREATE TABLE IF NOT EXISTS samples (
    machine_id TEXT NOT NULL,
    ts         INTEGER NOT NULL,       -- unix seconds
    cpu        REAL,                   -- percent 0-100
    mem_used   INTEGER,                -- bytes
    disk_used  INTEGER,                -- bytes
    disk_total INTEGER,                -- bytes
    load1      REAL,
    uptime     INTEGER,                -- seconds
    battery    REAL,                   -- percent, null when no battery
    latency_ms REAL,                   -- probe round-trip
    online     INTEGER                 -- probe reachability (1/0)
  );
  CREATE INDEX IF NOT EXISTS samples_by_machine ON samples (machine_id, ts);
`);

const upsertMachine = db.query(`
  INSERT INTO machines (id, kind, hostname, platform, arch, cpus, mem_total, first_seen, last_seen)
  VALUES ($id, $kind, $hostname, $platform, $arch, $cpus, $mem_total, $now, $now)
  ON CONFLICT(id) DO UPDATE SET
    hostname = excluded.hostname, platform = excluded.platform, arch = excluded.arch,
    cpus = excluded.cpus, mem_total = excluded.mem_total, last_seen = excluded.last_seen
`);
const insertSample = db.query(`
  INSERT INTO samples (machine_id, ts, cpu, mem_used, disk_used, disk_total, load1, uptime, battery, latency_ms, online)
  VALUES ($machine_id, $ts, $cpu, $mem_used, $disk_used, $disk_total, $load1, $uptime, $battery, $latency_ms, $online)
`);
const listMachines = db.query(`SELECT * FROM machines ORDER BY kind, hostname`);
const latestSample = db.query(`
  SELECT * FROM samples WHERE machine_id = $id ORDER BY ts DESC LIMIT 1
`);
const recentSamples = db.query(`
  SELECT ts, cpu, latency_ms, online FROM samples
  WHERE machine_id = $id AND ts >= $since ORDER BY ts
`);
const historySamples = db.query(`
  SELECT ts, cpu, mem_used, disk_used, disk_total, load1, battery, latency_ms, online
  FROM samples WHERE machine_id = $id AND ts >= $since ORDER BY ts
`);

const now = () => Math.floor(Date.now() / 1000);

function sweepRetention() {
  db.query(`DELETE FROM samples WHERE ts < $cutoff`).run({ $cutoff: now() - RETENTION_HOURS * 3600 });
}
setInterval(sweepRetention, 3600_000);
sweepRetention();

// --- Ingest ------------------------------------------------------------------

// Finite number or null — rejects NaN/Infinity/strings without throwing.
const num = (v: unknown): number | null => (typeof v === "number" && Number.isFinite(v) ? v : null);

function acceptReport(body: unknown): string | null {
  const r = body as Record<string, unknown>;
  if (!r || typeof r.id !== "string" || r.id === "" || typeof r.hostname !== "string") {
    return "report must include string id and hostname";
  }
  const ts = now();
  upsertMachine.run({
    $id: r.id,
    $kind: "agent",
    $hostname: r.hostname,
    $platform: typeof r.platform === "string" ? r.platform : null,
    $arch: typeof r.arch === "string" ? r.arch : null,
    $cpus: num(r.cpus),
    $mem_total: num(r.mem_total),
    $now: ts,
  });
  insertSample.run({
    $machine_id: r.id,
    $ts: ts,
    $cpu: num(r.cpu),
    $mem_used: num(r.mem_used),
    $disk_used: num(r.disk_used),
    $disk_total: num(r.disk_total),
    $load1: num(r.load1),
    $uptime: num(r.uptime),
    $battery: num(r.battery),
    $latency_ms: null,
    $online: null,
  });
  return null;
}

// --- Agentless probes ----------------------------------------------------------

function probeTcp(host: string, port: number, timeoutMs = 2500): Promise<number | null> {
  const { promise, resolve } = Promise.withResolvers<number | null>();
  const start = performance.now();
  let settled = false;
  const done = (ms: number | null) => {
    if (!settled) {
      settled = true;
      resolve(ms);
    }
  };
  const timer = setTimeout(() => done(null), timeoutMs);
  Bun.connect({
    hostname: host,
    port,
    socket: {
      open(s) {
        clearTimeout(timer);
        const ms = performance.now() - start;
        s.end();
        done(ms);
      },
      error() {
        clearTimeout(timer);
        done(null);
      },
      connectError() {
        clearTimeout(timer);
        done(null);
      },
      data() {},
      close() {},
    },
  }).catch(() => {
    clearTimeout(timer);
    done(null);
  });
  return promise;
}

async function probePing(host: string): Promise<number | null> {
  // Timeout flags differ: -t is TTL on Linux but timeout on macOS.
  const args =
    process.platform === "darwin"
      ? ["ping", "-n", "-c", "1", "-t", "2", host]
      : ["ping", "-n", "-c", "1", "-W", "2", host];
  try {
    const proc = Bun.spawn(args, { stdout: "pipe", stderr: "ignore" });
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    if (proc.exitCode !== 0) return null;
    const m = out.match(/time[=<]([\d.]+)/);
    return m ? Number(m[1]) : 0;
  } catch {
    return null;
  }
}

async function probeSweep() {
  await Promise.all(
    PROBES.map(async (t) => {
      const latency = t.port ? await probeTcp(t.host, t.port) : await probePing(t.host);
      const ts = now();
      upsertMachine.run({
        $id: `probe:${t.name}`,
        $kind: "probe",
        $hostname: t.name,
        $platform: t.port ? `tcp:${t.host}:${t.port}` : `ping:${t.host}`,
        $arch: null,
        $cpus: null,
        $mem_total: null,
        $now: ts,
      });
      insertSample.run({
        $machine_id: `probe:${t.name}`,
        $ts: ts,
        $cpu: null,
        $mem_used: null,
        $disk_used: null,
        $disk_total: null,
        $load1: null,
        $uptime: null,
        $battery: null,
        $latency_ms: latency,
        $online: latency === null ? 0 : 1,
      });
    }),
  );
}

// --- Fleet view ----------------------------------------------------------------

type SparkRow = { ts: number; cpu: number | null; latency_ms: number | null; online: number | null };

// Bucket the last hour into SPARK_POINTS averages of the machine's key series
// (cpu for agents, latency for probes). Unreachable probe samples become 0-height.
function sparkline(id: string, kind: string): (number | null)[] {
  const since = now() - 3600;
  const rows = recentSamples.all({ $id: id, $since: since }) as SparkRow[];
  const buckets: { sum: number; n: number }[] = Array.from({ length: SPARK_POINTS }, () => ({ sum: 0, n: 0 }));
  const width = 3600 / SPARK_POINTS;
  for (const r of rows) {
    const v = kind === "probe" ? (r.online === 0 ? 0 : r.latency_ms) : r.cpu;
    if (v === null || v === undefined) continue;
    const i = Math.min(SPARK_POINTS - 1, Math.floor((r.ts - since) / width));
    buckets[i].sum += v;
    buckets[i].n += 1;
  }
  return buckets.map((b) => (b.n === 0 ? null : b.sum / b.n));
}

function machineStatus(kind: string, lastSeen: number, latest: Record<string, unknown> | null): string {
  const age = now() - lastSeen;
  if (kind === "probe") {
    if (age > PROBE_INTERVAL * 3) return "stale"; // hub itself was down
    return latest && latest.online === 1 ? "online" : "offline";
  }
  if (age <= STALE_AFTER) return "online";
  if (age <= OFFLINE_AFTER) return "stale";
  return "offline";
}

function fleetView() {
  const machines = (listMachines.all() as Record<string, unknown>[]).map((m) => {
    const latest = latestSample.get({ $id: m.id }) as Record<string, unknown> | null;
    return {
      id: m.id,
      kind: m.kind,
      hostname: m.hostname,
      platform: m.platform,
      arch: m.arch,
      cpus: m.cpus,
      mem_total: m.mem_total,
      first_seen: m.first_seen,
      last_seen: m.last_seen,
      status: machineStatus(m.kind as string, m.last_seen as number, latest),
      latest,
      spark: sparkline(m.id as string, m.kind as string),
    };
  });
  return { generated: now(), probe_interval: PROBE_INTERVAL, machines };
}

// --- HTTP server -----------------------------------------------------------------

const UI = Bun.file(new URL("./ui.html", import.meta.url).pathname);

const json = (data: unknown, status = 200) =>
  new Response(JSON.stringify(data), { status, headers: { "content-type": "application/json" } });

Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);

    if (req.method === "GET" && url.pathname === "/") {
      return new Response(UI, { headers: { "content-type": "text/html; charset=utf-8" } });
    }
    if (req.method === "GET" && url.pathname === "/healthz") {
      return json({ ok: true });
    }
    if (req.method === "GET" && url.pathname === "/api/fleet") {
      return json(fleetView());
    }
    if (req.method === "GET" && url.pathname.startsWith("/api/machine/")) {
      const id = decodeURIComponent(url.pathname.slice("/api/machine/".length));
      const hours = Math.min(RETENTION_HOURS, Math.max(1, Number(url.searchParams.get("hours") ?? 24)));
      const rows = historySamples.all({ $id: id, $since: now() - hours * 3600 });
      return json({ id, hours, samples: rows });
    }
    if (req.method === "POST" && url.pathname === "/api/report") {
      if (TOKEN !== "" && req.headers.get("authorization") !== `Bearer ${TOKEN}`) {
        return json({ error: "unauthorized" }, 401);
      }
      let body: unknown;
      try {
        body = await req.json();
      } catch {
        return json({ error: "invalid JSON" }, 400);
      }
      const err = acceptReport(body);
      return err ? json({ error: err }, 400) : json({ ok: true });
    }
    return json({ error: "not found" }, 404);
  },
});

console.log(
  `fleet hub listening on :${PORT} (db: ${DB_PATH}, probes: ${PROBES.length}, auth: ${TOKEN ? "token" : "open"})`,
);

// Kick off probes only once the server is listening, so a self-probe of the
// hub's own port doesn't race the bind.
if (PROBES.length > 0) {
  probeSweep();
  setInterval(probeSweep, PROBE_INTERVAL * 1000);
}
