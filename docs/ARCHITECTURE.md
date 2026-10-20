# Public architecture

[English](ARCHITECTURE.md) | [简体中文](zh-CN/ARCHITECTURE.md)

See [EDITIONS](EDITIONS.md) for repository dependencies and version boundaries.

| Directory | Responsibility |
|---|---|
| `crates/server-core` | Axum identity/resource APIs, SQLite schema 4, optional rate gateways, Caddy/rathole coordination, local nodes, backup and restore |
| `apps/selfhost` | Public server assembled with SelfHosted policy, without Cloud application dependencies |
| `crates/client-core` | Device binding, configuration synchronization, leases, logs, and child-process lifecycle |
| `apps/cli` | Shared command-line client and background service installation |
| `apps/desktop` | Optional Tauri app using the client library; separate Cargo workspace |
| `web` | Shared console selected by capabilities, plus the shared localization runtime and catalogs |

Cloud implements multi-user handlers in its own Apache-2.0 repository and extends the API through `router_with_routes`, pinned to a public commit. Public code neither imports nor assembles Cloud routes. UI capability checks control presentation; the server enforces authorization independently.

## Data and rate limits

Devices and tunnels share resource IDs and ownership. Self-hosted uses one administrator, without count or total bandwidth quotas. Schema 4 records unlimited tunnels in `tunnel_unlimited`; existing finite-limit storage remains compatible and 0 still means disabled.

Unlimited tunnels bypass rate waits while retaining connection/queue protection, authorization revocation, and leases. Finite tunnels share a budget across both directions and all connections. Opening more connections cannot multiply that budget. There is no cumulative traffic persistence or billing.

## Interface language

English source messages are keys in `web/locales/en.json`; locale catalogs supply translations. The embedded web console and desktop app share the offline runtime and generated catalog bundle. Desktop packaging copies those assets; tray labels come from the same catalogs. API identifiers and serialized fields are language independent. See [I18N](I18N.md).

## Node evolution

Only LocalNode is implemented. Global node discovery, network-quality measurement, routing selection, and failover are future work. Extend the public node protocol first; Cloud can orchestrate operational policy in its Cloud control plane.
