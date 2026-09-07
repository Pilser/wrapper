# Wrapper

The **host-side orchestrator** for the AI Learning Companion platform. The wrapper
runs on the physical host machine and manages a fleet of
[Firecracker](https://firecracker-microvm.github.io/) microVMs — one isolated VM per
student (tenant). It authenticates users, routes traffic to the right VM, owns all
databases, and exposes admin controls.

The wrapper is the **brain** of the system; the
[supervisor](https://github.com) (a separate repo) runs inside each VM as the **hands**.

## Architecture

```
[Student browser] ──WebSocket/HTTP──▶ [Wrapper (this repo)]
                                          │
                                          │  manages
                                          ▼
                                    [Firecracker microVMs]
                                          │  each runs
                                          ▼
                                    [Supervisor (guest daemon)]
                                          │  runs
                                          ▼
                                    [AI Agent + Memory binary]
```

**Request flow:** student connects → wrapper authenticates → wrapper routes to the
student's VM → the agent inside the VM processes the request → the agent asks the
supervisor for data → the supervisor asks the wrapper → the wrapper reads/writes the
database → the response flows back to the student.

## Tech stack

- **Language:** Rust (edition 2024)
- **HTTP / WebSocket server:** [`hyper`](https://hyper.rs) (not axum)
- **Database:** [Turso](https://turso.tech/) (`turso` crate `0.8.0-pre.8`) — a
  SQLite-compatible, in-process database. One **global** database plus one
  **per-tenant** database file (Turso can spawn many local DB files).
- **VM runtime:** Firecracker microVMs (v1.7.0), controlled via a minimal
  HTTP/1.1-over-Unix-socket client.
- **Async runtime:** Tokio
- **Serialization:** serde / serde_json

## Workspace structure

Each crate is small and focused, matching the planned folder layout so no single
file is overloaded. Crates are listed in dependency order (implement top-down):

| # | Crate | Responsibility |
|---|-------|----------------|
| 1 | `core` | Shared types (`Tenant`, `VmInfo`, `Session`, `Category`, …), the `Plugin` trait, `Error`/`Result`, time/fs/validation utilities |
| 2 | `telemetry` | Structured events + metrics: a single `Telemetry` handle, daily JSON-line disk files, live WebSocket broadcast, counters/gauges/histograms, query/export |
| 3 | `db` | Global + per-tenant Turso databases, migrations, a parameterized query builder |
| 4 | `network` | Tap devices, the tenant IP pool, vsock, bridge, NAT and firewall rules |
| 5 | `vm` | Firecracker VM lifecycle (boot/shutdown/reboot/snapshot/restore/migrate), templates, idle tracking, resource limits |
| 6 | `proxy` | HTTP + WebSocket proxy between clients and agents (hyper-based) |
| 7 | `auth` | Login, token minting/refresh/revocation, sessions, roles/permissions, OAuth + MFA stubs, audit log |
| 8 | `supervisor_server` | Receives MCP/data calls from supervisors, serves tenant tools, handles heartbeats and deployments |
| 9 | `admin` | Categories, VM templates, deployments, tenant management, staff, dashboard stats |
| 10 | `plugin_host` | Loads `.so` plugins via `libloading`, folder watch + hot-reload, registry, hook dispatch, inter-plugin API |
| 11 | `api` | Public plugin-facing API surface that ties every subsystem together |

The binary lives in `src/` (`main.rs` + `config/`, `shutdown/`, `logging/`).

## Plugin system

Every crate exposes a **plugin API**. Plugins are `.so` files loaded at runtime that can:
register custom MCP tools, telemetry events and admin widgets; hook into any lifecycle
event; and access the databases, VMs, networks and proxy. The contract is defined in
`core` (`Plugin`, `PluginContext`, `PluginInfo`, `HookPoint`).

## Telemetry

Every event follows one JSON schema (id, timestamp with nanosecond precision, level,
dotted `type`, category, source, optional tenant/vm/session/trace/span ids, message,
`data`, tags, and a typed payload). Events are written to daily files under
`/var/log/wrapper/telemetry/` and can be broadcast live to dashboards.

## Databases

- **Global DB** (`data/global.db`): `categories`, `vm_templates`, `tenants`,
  `category_assignments`, `deployment_configs`, `networks`, `sessions`.
- **Per-tenant DB** (`data/tenants/<id>.db`): `profiles`, `memory`, `history`,
  `projects`, `skills`, `reports`, `configs`.

## Configuration

`config.toml` at the repo root holds every setting (bind addresses, Firecracker path,
network range, database paths, supervisor server, plugins, categories, telemetry).

## Build & verify

```bash
# Only ever run cargo check (never build/test/run)
cargo check --workspace
```

> Rule: verification is by `cargo check` only. The goal is to confirm the code
> compiles, not to run it.

## License

TBD
