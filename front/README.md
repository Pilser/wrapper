# front — bundled web UI for the wrapper

This folder holds the frontend for the wrapper's `/api` surface. It is
tracked in this repo so the wrapper binary can serve it directly from
`front/dist` — no separate hosting needed.

```
wrapper/
├── front/
│   ├── src/            React + TypeScript source (Vite)
│   ├── index.html
│   ├── package.json    deps: react, react-dom, lucide-react
│   ├── vite.config.ts  dev proxy -> https://apiai.pilserlabs.com
│   └── dist/           BUILD OUTPUT — committed; the wrapper serves this
```

## Build (required before running the wrapper in production)

```bash
cd front
bun install
bun run build       # typechecks then writes front/dist/
```

That's it. `front/dist` is plain static files (`index.html` +
`assets/index-<hash>.{js,css}`). If you change UI source, rebuild and the
new hashes land in dist; commit both.

## How the wrapper serves it

Configured in the root **`config.toml`**:

```toml
[front]
dist = "front/dist"
```

Wiring, in order:

1. **Config** — `src/config/mod.rs` (`FrontSection`) + `src/config/parse.rs`
   read `[front].dist` (default `front/dist`).
2. **Startup** — `src/main.rs` passes it into `api::server::ServeParams.front_dist`.
3. **Load once** — `crates/api/src/server/front.rs` (`Front::load`) walks the
   dist folder at startup and keeps every file in memory (~350 KB). Missing
   folder or missing `index.html` = front disabled, pure proxy mode (logged).
4. **Request routing** — `crates/api/src/server/mod.rs` (`serve`): the SAME
   hyper listener that serves `/api/*` checks, for `GET`/`HEAD` only:
   - `/` or `/index.html` → serve `index.html` (no-cache)
   - exact asset match (`/assets/*` etc.) → serve file (immutable, 1-year
     cache — vite hashes filenames)
   - **anything else** → falls through to the tenant zw proxy untouched
5. The UI uses **hash routing** (`#/`, `#/admin/...`), so only `/` plus real
   asset paths need serving — no SPA fallback, no server-side rewrites.

## Proxy safety guarantees (why nothing else breaks)

The static handler only claims a request when ALL of these hold:

- method is `GET` or `HEAD` (uploads POST/PUT/DELETE always proxy),
- there is **no `Upgrade: websocket` header** (SSE/WS stream through),
- the path exactly matches a file that exists in the dist map.

Everything else is untouched:

| Traffic | Path |
|---|---|
| `/api/*` (REST, admin WS stream + events, binary uploads) | local `/api` handlers, as before |
| tenant zw apps (its own `/api/sessions`, pages, WS tunnels) | proxy, unchanged |
| downloads, unknown paths, all POST/PUT/DELETE | proxy, unchanged |

## UI feature map (what's wired)

Auth model: tenant Bearer token + staff Bearer token, kept separately
(localStorage); WS feeds pass the staff token via `?token=`.

- **Tenant**: signup, login (VM pre-warm server-side), logout, dashboard
  (`GET /api/me` + `GET /api/me/vm/status`, 5s poll, hiccup-resilient),
  wake (`POST /api/me/vm/wake`), apps runner (`#/apps` fullscreen iframe of
  the tenant's `apps_url` with a single back-to-dashboard bar).
- **Admin console** (staff login + first-run bootstrap):
  - overview: `WS /api/admin/stream?tenant=<id|all>` with REST fallback
    (`GET /api/admin/dashboard`), username resolution, mem/cpu/disk/heartbeat
  - tenants: CRUD, suspend/unsuspend, disk view, prompt sync, program push
  - categories: CRUD, tool grants (`/tools`, catalog), deployment configs
  - templates: CRUD (kernel/initrd/vcpu/mem/gpu)
  - machines (fleet): list + sleep/stop/shutdown/kill, host disk list
  - programs (binaries): manifest, octet-stream upload with client
    SHA-256 (`X-Content-Sha256`), staging, diff, merge+version, rollback,
    push/push-bulk, sync/sync-bulk, effective-per-tenant
  - instructions: system prompt scope CRUD, bulk distribution, effective
    probe; zw/engram launcher scripts
  - sharing (hub grants): view/issue/revoke
  - events (telemetry): `GET /api/admin/logs` filters + `WS /api/admin/events`
    live feed, expandable rows
- No user-facing restart endpoint exists in the wrapper API (restarts are
  internal to admin sync/push operations); the UI states this plainly.

## Production runbook (for the next agent)

```bash
git clone https://github.com/Pilser/wrapper && cd wrapper
cd front && bun install && bun run build && cd ..   # produce front/dist
cargo build --release                                # needs clang (see .cargo/config.toml)
# edit config.toml: [api] public_base, [front] dist, [docker], ...
./target/release/wrapper                             # serves UI at http://host:8080/
```

- The UI calls the API same-origin (`/api/...`), so browsers need no CORS
  setup when the wrapper serves it. When the UI is hosted elsewhere instead,
  keep the `[front]` dist absent and rely on the wrapper's CORS headers.
- Dev mode for UI work: `bun run dev` in `front/` (Vite proxies `/api`
  incl. WS to `https://apiai.pilserlabs.com`).
- Theme: forest-dark default, cream-light toggle (persisted per browser).
