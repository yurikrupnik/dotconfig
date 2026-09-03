// devkit manager — role-gated cluster dashboard for local kind clusters.
//
// Zero dependencies; runs on Bun. Talks to the Kubernetes API either
// in-cluster (ServiceAccount token + CA) or, for local development, through
// `kubectl proxy` (K8S_API, default http://127.0.0.1:8001).
//
// Trust model: this is a LOCAL DEV tool. The requesting role arrives as the
// `x-manager-role` header (picked in the UI); the server enforces the panel
// grants from roles.ts, so a role never receives data outside its grants —
// but there is no authentication. Do not deploy beyond a local kind cluster.

import { GRANTS, Panel, Role, parseRole } from "./roles.ts";

const SA_DIR = "/var/run/secrets/kubernetes.io/serviceaccount";
const PORT = Number(Bun.env.PORT ?? 8300);

// --- Narrow views of the K8s API objects we consume -------------------------

type Meta = {
  name: string;
  namespace?: string;
  labels?: Record<string, string>;
  creationTimestamp?: string;
};
type Condition = { type: string; status: string };
type List<T> = { items: T[] };

type K8sNode = {
  metadata: Meta;
  spec?: { taints?: { key: string; value?: string; effect: string }[] };
  status?: {
    conditions?: Condition[];
    capacity?: Record<string, string>;
    nodeInfo?: {
      kubeletVersion?: string;
      operatingSystem?: string;
      architecture?: string;
    };
  };
};
type Pod = { metadata: Meta; status?: { phase?: string } };
type Deployment = {
  metadata: Meta;
  spec?: { replicas?: number; template?: { spec?: { containers?: { image: string }[] } } };
  status?: { readyReplicas?: number };
};
type K8sEvent = {
  metadata: Meta;
  type?: string;
  reason?: string;
  message?: string;
  count?: number;
  lastTimestamp?: string;
  eventTime?: string;
  involvedObject?: { kind?: string; name?: string; namespace?: string };
};
type Secret = { metadata: Meta; type?: string; data?: Record<string, string> };
type ClusterRoleBinding = {
  metadata: Meta;
  roleRef?: { name?: string };
  subjects?: { kind: string; name: string }[];
};

// --- K8s API client ----------------------------------------------------------

type K8sClient = { base: string; init: RequestInit };

async function k8sClient(): Promise<K8sClient> {
  const tokenFile = Bun.file(`${SA_DIR}/token`);
  if (await tokenFile.exists()) {
    const [token, ca] = await Promise.all([
      tokenFile.text(),
      Bun.file(`${SA_DIR}/ca.crt`).text(),
    ]);
    // `tls` is a Bun fetch extension absent from the standard RequestInit type.
    const init = {
      headers: { authorization: `Bearer ${token.trim()}` },
      tls: { ca },
    } as RequestInit;
    return {
      base: `https://${Bun.env.KUBERNETES_SERVICE_HOST}:${Bun.env.KUBERNETES_SERVICE_PORT ?? 443}`,
      init,
    };
  }
  return { base: Bun.env.K8S_API ?? "http://127.0.0.1:8001", init: {} };
}

const k8s = await k8sClient();

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${k8s.base}${path}`, k8s.init);
  if (!res.ok) throw new Error(`${path} -> ${res.status} ${await res.text()}`);
  // Trusted local API server; shape asserted by the narrow domain types above.
  return (await res.json()) as T;
}

// ---------------------------------------------------------------------------
// Panel collectors — each returns the JSON payload for one dashboard panel.
// ---------------------------------------------------------------------------

async function collectSummary() {
  const [nodes, namespaces, pods, deployments] = await Promise.all([
    get<List<K8sNode>>("/api/v1/nodes"),
    get<List<{ metadata: Meta }>>("/api/v1/namespaces"),
    get<List<Pod>>("/api/v1/pods"),
    get<List<Deployment>>("/apis/apps/v1/deployments"),
  ]);
  const phases: Record<string, number> = {};
  for (const p of pods.items) {
    const phase = p.status?.phase ?? "Unknown";
    phases[phase] = (phases[phase] ?? 0) + 1;
  }
  const ready = nodes.items.filter((n) =>
    n.status?.conditions?.some((c) => c.type === "Ready" && c.status === "True"),
  ).length;
  return {
    nodes: { total: nodes.items.length, ready },
    namespaces: namespaces.items.length,
    pods: { total: pods.items.length, phases },
    deployments: deployments.items.length,
  };
}

async function collectNodes() {
  const nodes = await get<List<K8sNode>>("/api/v1/nodes");
  return nodes.items.map((n) => ({
    name: n.metadata.name,
    roles: Object.keys(n.metadata.labels ?? {})
      .filter((l) => l.startsWith("node-role.kubernetes.io/"))
      .map((l) => l.split("/")[1]),
    version: n.status?.nodeInfo?.kubeletVersion,
    os: `${n.status?.nodeInfo?.operatingSystem}/${n.status?.nodeInfo?.architecture}`,
    cpu: n.status?.capacity?.cpu,
    memory: n.status?.capacity?.memory,
    taints: (n.spec?.taints ?? []).map((t) => `${t.key}=${t.value ?? ""}:${t.effect}`),
    ready: n.status?.conditions?.some((c) => c.type === "Ready" && c.status === "True") ?? false,
  }));
}

async function collectWorkloads() {
  const [deployments, pods] = await Promise.all([
    get<List<Deployment>>("/apis/apps/v1/deployments"),
    get<List<Pod>>("/api/v1/pods"),
  ]);
  const podsByNamespace: Record<string, Record<string, number>> = {};
  for (const p of pods.items) {
    const ns = p.metadata.namespace ?? "";
    const phase = p.status?.phase ?? "Unknown";
    podsByNamespace[ns] = podsByNamespace[ns] ?? {};
    podsByNamespace[ns][phase] = (podsByNamespace[ns][phase] ?? 0) + 1;
  }
  return {
    deployments: deployments.items.map((d) => ({
      namespace: d.metadata.namespace,
      name: d.metadata.name,
      ready: `${d.status?.readyReplicas ?? 0}/${d.spec?.replicas ?? 0}`,
      images: (d.spec?.template?.spec?.containers ?? []).map((c) => c.image),
    })),
    podsByNamespace,
  };
}

async function collectEvents() {
  const events = await get<List<K8sEvent>>("/api/v1/events?limit=100");
  return events.items
    .map((e) => ({
      type: e.type ?? "Normal",
      reason: e.reason ?? "",
      object: `${e.involvedObject?.kind}/${e.involvedObject?.name}`,
      namespace: e.involvedObject?.namespace ?? "",
      message: e.message ?? "",
      count: e.count ?? 1,
      last: e.lastTimestamp ?? e.eventTime ?? e.metadata.creationTimestamp ?? "",
    }))
    .sort((a, b) => {
      if (a.type !== b.type) return a.type === "Warning" ? -1 : 1;
      return b.last.localeCompare(a.last);
    })
    .slice(0, 40);
}

async function collectSecrets() {
  const secrets = await get<List<Secret>>("/api/v1/secrets");
  // Metadata only — values are never surfaced.
  return secrets.items.map((s) => ({
    namespace: s.metadata.namespace,
    name: s.metadata.name,
    type: s.type,
    keys: Object.keys(s.data ?? {}).length,
    age: s.metadata.creationTimestamp,
  }));
}

async function collectRbac() {
  const [roles, bindings] = await Promise.all([
    get<List<{ metadata: Meta }>>("/apis/rbac.authorization.k8s.io/v1/clusterroles"),
    get<List<ClusterRoleBinding>>("/apis/rbac.authorization.k8s.io/v1/clusterrolebindings"),
  ]);
  return {
    clusterRoles: roles.items.length,
    bindings: bindings.items
      .filter((b) => !b.metadata.name.startsWith("system:"))
      .map((b) => ({
        name: b.metadata.name,
        role: b.roleRef?.name,
        subjects: (b.subjects ?? []).map((s) => `${s.kind}:${s.name}`),
      })),
  };
}

const COLLECTORS: Record<Panel, () => Promise<unknown>> = {
  [Panel.Summary]: collectSummary,
  [Panel.Nodes]: collectNodes,
  [Panel.Workloads]: collectWorkloads,
  [Panel.Events]: collectEvents,
  [Panel.Secrets]: collectSecrets,
  [Panel.Rbac]: collectRbac,
};

// ---------------------------------------------------------------------------
// HTTP server
// ---------------------------------------------------------------------------

const UI = Bun.file(new URL("./ui.html", import.meta.url).pathname);

Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(UI, { headers: { "content-type": "text/html; charset=utf-8" } });
    }

    if (url.pathname === "/healthz") return new Response("ok");

    if (url.pathname === "/api/roles") {
      return Response.json({ roles: Object.values(Role), grants: GRANTS });
    }

    if (url.pathname === "/api/overview") {
      const role = parseRole(req.headers.get("x-manager-role"));
      if (role === null) {
        return Response.json(
          { error: `unknown role; expected one of: ${Object.values(Role).join(", ")}` },
          { status: 403 },
        );
      }
      const panels: Record<string, unknown> = {};
      const results = await Promise.allSettled(
        GRANTS[role].map(async (panel) => {
          panels[panel] = await COLLECTORS[panel]();
        }),
      );
      const errors = results
        .filter((r): r is PromiseRejectedResult => r.status === "rejected")
        .map((r) => String(r.reason));
      return Response.json({ role, panels, ...(errors.length ? { errors } : {}) });
    }

    return new Response("not found", { status: 404 });
  },
});

console.log(`devkit manager listening on :${PORT} (k8s: ${k8s.base})`);
