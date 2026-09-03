// devkit fleet agent — pushes machine snapshots to the fleet hub.
//
// Zero dependencies; runs on Bun on macOS and Linux. Push (not pull) so
// sleeping laptops and NAT'd machines simply go stale on the hub instead of
// failing scrapes. Metrics come from node:os plus a few boring platform
// commands (vm_stat / /proc/meminfo, df, pmset / /sys battery).
//
// Env: FLEET_HUB (default http://localhost:9300), FLEET_TOKEN, FLEET_INTERVAL.
// `--once` sends a single report and exits (smoke tests, cron).

import os from "node:os";

const HUB = (Bun.env.FLEET_HUB ?? "http://localhost:9300").replace(/\/+$/, "");
const TOKEN = Bun.env.FLEET_TOKEN ?? "";
const INTERVAL = Math.max(2, Number(Bun.env.FLEET_INTERVAL ?? 10));
const ONCE = Bun.argv.includes("--once");

async function run(cmd: string[]): Promise<string | null> {
  try {
    const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "ignore" });
    const out = await new Response(proc.stdout).text();
    await proc.exited;
    return proc.exitCode === 0 ? out : null;
  } catch {
    return null;
  }
}

// --- CPU: percent busy between consecutive samples ---------------------------

function cpuTimes(): { idle: number; total: number } {
  let idle = 0;
  let total = 0;
  for (const c of os.cpus()) {
    idle += c.times.idle;
    total += c.times.user + c.times.nice + c.times.sys + c.times.idle + c.times.irq;
  }
  return { idle, total };
}

let prevCpu = cpuTimes();

function cpuPercent(): number | null {
  const cur = cpuTimes();
  const dTotal = cur.total - prevCpu.total;
  const dIdle = cur.idle - prevCpu.idle;
  prevCpu = cur;
  if (dTotal <= 0) return null;
  return Math.max(0, Math.min(100, (1 - dIdle / dTotal) * 100));
}

// --- Memory: really-used bytes (os.freemem undercounts badly on macOS) --------

async function memUsed(): Promise<number | null> {
  if (process.platform === "darwin") {
    const out = await run(["vm_stat"]);
    if (out) {
      const pageSize = Number(out.match(/page size of (\d+) bytes/)?.[1] ?? 16384);
      let pages = 0;
      for (const key of ["Pages active", "Pages wired down", "Pages occupied by compressor"]) {
        const m = out.match(new RegExp(`${key}:\\s+(\\d+)`));
        if (m) pages += Number(m[1]);
      }
      if (pages > 0) return pages * pageSize;
    }
  }
  if (process.platform === "linux") {
    try {
      const meminfo = await Bun.file("/proc/meminfo").text();
      const total = Number(meminfo.match(/MemTotal:\s+(\d+)/)?.[1]);
      const avail = Number(meminfo.match(/MemAvailable:\s+(\d+)/)?.[1]);
      if (Number.isFinite(total) && Number.isFinite(avail)) return (total - avail) * 1024;
    } catch {}
  }
  return os.totalmem() - os.freemem();
}

// --- Disk: root filesystem usage (macOS: the data volume, not the sealed system) --

async function diskUsage(): Promise<{ disk_used: number; disk_total: number } | null> {
  const path = process.platform === "darwin" ? "/System/Volumes/Data" : "/";
  const out = await run(["df", "-k", path]);
  const cols = out?.split("\n")[1]?.trim().split(/\s+/);
  if (!cols || cols.length < 4) return null;
  const total = Number(cols[1]) * 1024;
  const used = Number(cols[2]) * 1024;
  if (!Number.isFinite(total) || !Number.isFinite(used)) return null;
  return { disk_used: used, disk_total: total };
}

// --- Battery: percent, null on desktops ----------------------------------------

async function batteryPercent(): Promise<number | null> {
  if (process.platform === "darwin") {
    const out = await run(["pmset", "-g", "batt"]);
    const m = out?.match(/(\d+)%/);
    return m ? Number(m[1]) : null;
  }
  if (process.platform === "linux") {
    for (const bat of ["BAT0", "BAT1"]) {
      try {
        const cap = await Bun.file(`/sys/class/power_supply/${bat}/capacity`).text();
        const n = Number(cap.trim());
        if (Number.isFinite(n)) return n;
      } catch {}
    }
  }
  return null;
}

// --- Report loop -----------------------------------------------------------------

async function report(): Promise<boolean> {
  const disk = await diskUsage();
  const snapshot = {
    id: os.hostname(),
    hostname: os.hostname(),
    platform: process.platform,
    arch: os.arch(),
    cpus: os.cpus().length,
    mem_total: os.totalmem(),
    cpu: cpuPercent(),
    mem_used: await memUsed(),
    disk_used: disk?.disk_used ?? null,
    disk_total: disk?.disk_total ?? null,
    load1: os.loadavg()[0],
    uptime: os.uptime(),
    battery: await batteryPercent(),
  };
  try {
    const res = await fetch(`${HUB}/api/report`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        ...(TOKEN ? { authorization: `Bearer ${TOKEN}` } : {}),
      },
      body: JSON.stringify(snapshot),
    });
    if (!res.ok) {
      console.error(`fleet agent: hub rejected report (${res.status}): ${await res.text()}`);
      return false;
    }
    return true;
  } catch (err) {
    console.error(`fleet agent: hub unreachable (${HUB}): ${err}`);
    return false;
  }
}

if (ONCE) {
  // cpuPercent needs two samples; give it a short window.
  await Bun.sleep(500);
  process.exit((await report()) ? 0 : 1);
}

console.log(`fleet agent: reporting ${os.hostname()} -> ${HUB} every ${INTERVAL}s`);
await Bun.sleep(500);
await report();
setInterval(report, INTERVAL * 1000);
