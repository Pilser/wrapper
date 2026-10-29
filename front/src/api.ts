// API client for the wrapper backend (/api on apiai.pilserlabs.com via the
// dev proxy). Shapes mirror the Rust handlers in crates/api/src/server.

export const KEYS = {
  tenant: "wrap.tenant_token",
  tenantSelf: "wrap.tenant_self",
  admin: "wrap.admin_token",
  adminSelf: "wrap.admin_self",
  api: "wrap.api_base",
  theme: "wrap.theme",
};

// Override base (e.g. https://apiai.pilserlabs.com) when not going through
// the dev proxy. Default "" = same origin (proxy handles it).
export function apiBase(): string {
  const v = localStorage.getItem(KEYS.api);
  return v ? v.replace(/\/+$/, "") : "";
}

export function setApiBase(v: string) {
  if (v.trim()) localStorage.setItem(KEYS.api, v.trim().replace(/\/+$/, ""));
  else localStorage.removeItem(KEYS.api);
}

export function tenantToken(): string {
  return localStorage.getItem(KEYS.tenant) ?? "";
}
export function adminToken(): string {
  return localStorage.getItem(KEYS.admin) ?? "";
}
export function saveTenantToken(tok: string, self: unknown) {
  localStorage.setItem(KEYS.tenant, tok);
  localStorage.setItem(KEYS.tenantSelf, JSON.stringify(self ?? null));
}
export function saveAdminToken(tok: string, self: unknown) {
  localStorage.setItem(KEYS.admin, tok);
  localStorage.setItem(KEYS.adminSelf, JSON.stringify(self ?? null));
}
export function clearTenantToken() {
  localStorage.removeItem(KEYS.tenant);
  localStorage.removeItem(KEYS.tenantSelf);
}
export function clearAdminToken() {
  localStorage.removeItem(KEYS.admin);
  localStorage.removeItem(KEYS.adminSelf);
}

export class ApiError extends Error {
  status: number;
  data: any;
  constructor(status: number, data: any) {
    // Cloudflare edge codes: origin (jerboai) down / unreachable / slow
    const cf: Record<number, string> = {
      520: "jerboai crashed while answering",
      521: "apiai wrapper is down right now",
      522: "apiai wrapper unreachable (timed out)",
      523: "apiai wrapper unreachable",
      524: "apiai wrapper took too long to answer",
    };
    if (cf[status]) {
      super(`${cf[status]} (Cloudflare ${status})`);
    } else {
      super((data && (data.error ?? data.detail)) || `HTTP ${status}`);
    }
    this.status = status;
    this.data = data;
  }
}

export type Resp = { status: number; data: any };

export async function call(
  path: string,
  opts: {
    method?: string;
    body?: any;
    admin?: boolean;
    tenant?: boolean;
    raw?: BodyInit;
    rawType?: string;
    headers?: Record<string, string>;
    query?: Record<string, string | undefined>;
  } = {}
): Promise<Resp> {
  const headers: Record<string, string> = { ...opts.headers };
  const tok = opts.admin ? adminToken() : opts.tenant ? tenantToken() : "";
  if (tok) headers["Authorization"] = `Bearer ${tok}`;
  if (opts.body !== undefined) headers["Content-Type"] = "application/json";
  if (opts.raw !== undefined && opts.rawType)
    headers["Content-Type"] = opts.rawType;
  const qs = opts.query
    ? "?" +
      Object.entries(opts.query)
        .filter(([, v]) => v !== undefined && v !== "")
        .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v!)}`)
        .join("&")
    : "";
  const res = await fetch(apiBase() + path + qs, {
    method: opts.method ?? (opts.body !== undefined || opts.raw !== undefined ? "POST" : "GET"),
    headers,
    body:
      opts.raw !== undefined
        ? opts.raw
        : opts.body !== undefined
        ? JSON.stringify(opts.body)
        : undefined,
  });
  const text = await res.text();
  let data: any = text;
  try {
    data = JSON.parse(text);
  } catch {
    /* raw text (octet-stream etc.) */
  }
  if (!res.ok) throw new ApiError(res.status, data);
  return { status: res.status, data };
}

export function wsUrl(path: string, query: Record<string, string | undefined>) {
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const host = apiBase() ? new URL(apiBase()).host : location.host;
  const qs = Object.entries(query)
    .filter(([, v]) => v !== undefined && v !== "")
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v!)}`)
    .join("&");
  const tok = adminToken();
  // WS can't set headers; the wrapper accepts ?token= for browsers.
  return `${proto}://${host}${path}?${qs}${qs ? "&" : ""}token=${encodeURIComponent(tok)}`;
}

// ------------------------------------------------------------- convenience

export const api = {
  // --- tenant auth/self
  signup: (b: any) => call("/api/auth/signup", { body: b }),
  login: (b: any) => call("/api/auth/login", { body: b }),
  logout: () => call("/api/auth/logout", { method: "POST", tenant: true }),
  me: () => call("/api/me", { tenant: true }),
  vmStatus: () => call("/api/me/vm/status", { tenant: true }),
  vmWake: () => call("/api/me/vm/wake", { method: "POST", tenant: true }),

  // --- admin auth
  bootstrap: (b: any) => call("/api/admin/bootstrap", { body: b, admin: true }),
  adminLogin: (b: any) => call("/api/admin/login", { body: b }),
  adminMe: () => call("/api/admin/me", { admin: true }),

  // --- admin tenants
  tenantsList: () => call("/api/admin/tenants", { admin: true }),
  tenantGet: (id: string) => call(`/api/admin/tenants/${id}`, { admin: true }),
  tenantCreate: (b: any) => call("/api/admin/tenants", { body: b, admin: true }),
  tenantUpdate: (id: string, b: any) =>
    call(`/api/admin/tenants/${id}`, { method: "PUT", body: b, admin: true }),
  tenantDelete: (id: string) =>
    call(`/api/admin/tenants/${id}`, { method: "DELETE", admin: true }),
  tenantSuspend: (id: string) =>
    call(`/api/admin/tenants/${id}/suspend`, { method: "POST", admin: true }),
  tenantUnsuspend: (id: string) =>
    call(`/api/admin/tenants/${id}/unsuspend`, { method: "POST", admin: true }),
  tenantDisk: (id: string) =>
    call(`/api/admin/tenants/${id}/disk`, { admin: true }),
  tenantDiskResize: (id: string, size_gb: number) =>
    call(`/api/admin/tenants/${id}/disk/resize`, { body: { size_gb }, admin: true }),

  // --- admin templates
  templatesList: () => call("/api/admin/templates", { admin: true }),
  templateGet: (id: string) => call(`/api/admin/templates/${id}`, { admin: true }),
  templateCreate: (b: any) => call("/api/admin/templates", { body: b, admin: true }),
  templateUpdate: (id: string, b: any) =>
    call(`/api/admin/templates/${id}`, { method: "PUT", body: b, admin: true }),
  templateDelete: (id: string) =>
    call(`/api/admin/templates/${id}`, { method: "DELETE", admin: true }),

  // --- admin categories
  categoriesList: () => call("/api/admin/categories", { admin: true }),
  categoryGet: (id: string) => call(`/api/admin/categories/${id}`, { admin: true }),
  categoryCreate: (b: any) => call("/api/admin/categories", { body: b, admin: true }),
  categoryUpdate: (id: string, b: any) =>
    call(`/api/admin/categories/${id}`, { method: "PUT", body: b, admin: true }),
  categoryDelete: (id: string) =>
    call(`/api/admin/categories/${id}`, { method: "DELETE", admin: true }),
  categoryTools: (id: string) =>
    call(`/api/admin/categories/${id}/tools`, { admin: true }),
  categoryToolsSet: (id: string, tools: string[]) =>
    call(`/api/admin/categories/${id}/tools`, { method: "PUT", body: { tools }, admin: true }),
  toolsCatalog: () => call("/api/admin/tools", { admin: true }),
  categoryConfigs: (id: string) =>
    call(`/api/admin/categories/${id}/configs`, { admin: true }),
  categoryConfigCreate: (id: string, b: any) =>
    call(`/api/admin/categories/${id}/configs`, { body: b, admin: true }),
  categoryConfigDelete: (id: string, cid: string) =>
    call(`/api/admin/categories/${id}/configs/${cid}`, { method: "DELETE", admin: true }),

  // --- fleet
  vmsList: () => call("/api/admin/vms", { admin: true }),
  vmAction: (id: string, action: string) =>
    call(`/api/admin/vms/${id}/${action}`, { method: "POST", admin: true }),
  disksList: () => call("/api/admin/disks", { admin: true }),

  // --- workspace / system prompt / launchers
  workspaceSync: (tenant_id: string) =>
    call("/api/admin/workspace/sync", { body: { tenant_id }, admin: true }),
  workspaceSyncBulk: (b: any) =>
    call("/api/admin/workspace/sync-bulk", { body: b, admin: true }),
  promptGet: (scope: string, ref: string) =>
    call("/api/admin/system-prompt", { admin: true, query: { scope, ref } }),
  promptSet: (b: any) =>
    call("/api/admin/system-prompt", { method: "PUT", body: b, admin: true }),
  promptDelete: (scope: string, ref: string) =>
    call("/api/admin/system-prompt", { method: "DELETE", admin: true, query: { scope, ref } }),
  promptEffective: (tenant_id: string) =>
    call("/api/admin/system-prompt/effective", { admin: true, query: { tenant_id } }),
  launchersGet: (scope: string, ref: string) =>
    call("/api/admin/launchers", { admin: true, query: { scope, ref } }),
  launcherSet: (b: any) =>
    call("/api/admin/launchers", { method: "PUT", body: b, admin: true }),
  launcherDelete: (scope: string, ref: string, name: string) =>
    call("/api/admin/launchers", { method: "DELETE", admin: true, query: { scope, ref, name } }),

  // --- binaries
  binariesList: () => call("/api/admin/binaries", { admin: true }),
  binaryUpload: (path: string, bytes: ArrayBuffer, stage: boolean, sha256: string) =>
    call("/api/admin/binaries/file", {
      method: "PUT",
      admin: true,
      raw: bytes,
      rawType: "application/octet-stream",
      query: { path: path.split("/").map(encodeURIComponent).join("/") },
      headers: { "X-Content-Sha256": sha256 },
    }),
  binaryPush: (tenant_id: string) =>
    call("/api/admin/binaries/push", { body: { tenant_id }, admin: true }),
  binaryPushBulk: (b: any) =>
    call("/api/admin/binaries/push-bulk", { body: b, admin: true }),
  binaryVersions: (name: string) =>
    call("/api/admin/binaries/versions", { admin: true, query: { name } }),
  binaryDiff: (name: string) =>
    call("/api/admin/binaries/diff", { body: { name }, admin: true }),
  binaryMerge: (name: string, note?: string) =>
    call("/api/admin/binaries/merge", { body: { name, note }, admin: true }),
  binaryRollback: (name: string, version_no: number) =>
    call("/api/admin/binaries/rollback", { body: { name, version_no }, admin: true }),
  binarySync: (tenant_id: string, name: string, version_no: number) =>
    call("/api/admin/binaries/sync", {
      body: { tenant_id, name, version_no },
      admin: true,
    }),
  binarySyncBulk: (b: any) =>
    call("/api/admin/binaries/sync-bulk", { body: b, admin: true }),
  binaryEffective: (tenant_id: string) =>
    call("/api/admin/binaries/effective", { admin: true, query: { tenant_id } }),

  // --- hub grants
  hubGrants: (owner: string) =>
    call("/api/admin/hub/grants", { admin: true, query: { owner } }),
  hubGrantCreate: (b: any) =>
    call("/api/admin/hub/grants", { body: b, admin: true }),
  hubGrantDelete: (b: any) =>
    call("/api/admin/hub/grants", { method: "DELETE", body: b, admin: true }),

  // --- dashboard / logs
  dashboard: (tenant: string) =>
    call("/api/admin/dashboard", { admin: true, query: { tenant } }),
  logs: (f: Record<string, string | undefined>) =>
    call("/api/admin/logs", { admin: true, query: f }),
};

export const BUNDLES = ["zw", "engram", "zw-assets"] as const;

export async function sha256Hex(buf: ArrayBuffer): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", buf);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function fmtBytes(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n < 1024) return `${n} B`;
  const u = ["KiB", "MiB", "GiB", "TiB"];
  let i = -1;
  let v = n;
  do {
    v /= 1024;
    i++;
  } while (v >= 1024 && i < u.length - 1);
  return `${v.toFixed(1)} ${u[i]}`;
}

export function ago(sec: number | null | undefined): string {
  if (sec == null) return "—";
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`;
  return `${Math.floor(sec / 86400)}d`;
}

export function statusClass(s: string | null | undefined): string {
  switch (s) {
    case "running":
    case "active":
      return "ok";
    case "asleep":
    case "sleeping":
    case "suspended":
    case "inactive":
      return "warn";
    case "unknown":
    case "stopped":
      return "muted";
    default:
      return "";
  }
}
