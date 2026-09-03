// Security model for the devkit manager dashboard.
//
// `Role` is the single source of truth: every viewer of the dashboard acts as
// exactly one role, and each role is granted a fixed set of panels. The server
// only fetches and returns data for panels the requesting role is granted —
// ungranted panels never leave the process.

export enum Role {
  Admin = "admin",
  Operator = "operator",
  Developer = "developer",
  Viewer = "viewer",
}

export enum Panel {
  Summary = "summary", // namespace/pod/node/deployment counts
  Nodes = "nodes", // node detail: versions, capacity, conditions
  Workloads = "workloads", // deployments + pod phases per namespace
  Events = "events", // recent cluster events, warnings first
  Secrets = "secrets", // secret metadata (names/types/key counts, never values)
  Rbac = "rbac", // clusterroles + bindings
}

export const GRANTS: Readonly<Record<Role, readonly Panel[]>> = {
  [Role.Viewer]: [Panel.Summary],
  [Role.Developer]: [Panel.Summary, Panel.Workloads, Panel.Events],
  [Role.Operator]: [Panel.Summary, Panel.Workloads, Panel.Events, Panel.Nodes],
  [Role.Admin]: [
    Panel.Summary,
    Panel.Workloads,
    Panel.Events,
    Panel.Nodes,
    Panel.Secrets,
    Panel.Rbac,
  ],
};

export function parseRole(value: string | null): Role | null {
  const v = (value ?? "").toLowerCase();
  return (Object.values(Role) as string[]).includes(v) ? (v as Role) : null;
}
