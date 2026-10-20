# Self-hosted API v1

This guide describes the current Self-hosted server. Except health probes, paths below use the `/api/v1` prefix. There is no open CORS policy. Request bodies are limited to 16 KiB. Application errors usually return `{"error":"English description"}`: 401 unauthenticated, 403 forbidden, 400 invalid input, 404 missing record, 409 conflict, 429 rate limited, or 502 edge failure. Framework parsing errors may be plain text. Error descriptions are human-readable diagnostics, not stable identifiers; clients should use HTTP status and operation context. GUI catalogs localize known messages and fall back to English for unknown diagnostics.

## Administrator sessions

First start initializes one administrator. Self-hosted has no registration, email verification/reset, or delegated user management. Login returns an `hp_session` cookie and CSRF value; later writes send `X-CSRF-Token`. Cookies are HttpOnly/Secure/SameSite=Strict and expire after 24 hours. `HP_ALLOW_HTTP=1` is only for local HTTP development.

| Method | Path | Request or response |
|---|---|---|
| GET | `/capabilities` | Public; edition, multi_user, quotas, email_auth, and other capabilities |
| POST | `/auth/login` | `{email,password}` → `{user,csrf}` and cookie |
| POST | `/auth/logout` | Current session and CSRF; sign out |
| GET | `/me` | user, profile, csrf, capabilities, resource counts, domain suffix, public connection address |
| PUT | `/me/profile` | `{display_name}`, at most 80 bytes |
| PUT | `/me/password` | `{current_password,password}`; sign in again after success |
| GET | `/me/sessions` | Own sessions `{id,current,expires}` |
| DELETE | `/me/sessions/{id}` | Revoke a selected session |

Passwords are 12–256 bytes; email addresses normalize to lowercase. Email is a local login name and needs no mail service. Session IDs are digests, not login tokens. Password changes do not revoke devices.

Self-hosted returns `null` for `limits.clients/tunnels/bps`, `allocated_bps`, and `remaining_bps`, meaning no product quotas. It rejects delegated `X-Manage-User` requests.

## Clients and tunnels

| Method | Path | Request or behavior |
|---|---|---|
| GET | `/portal/clients` | Workspace devices, without tokens |
| POST | `/portal/clients` | `{name}` → `{id,enroll_token,expires_in}` |
| POST | `/portal/clients/{id}/enrollment` | Regenerate a code; redemption rotates device and service tokens |
| POST | `/portal/clients/{id}/revoke` | Revoke the device and delete its tunnels |
| POST | `/enroll` | No web session; redeem `{token}` for `{client_id,token}` |
| GET | `/portal/tunnels` | Tunnels including bandwidth_limit_bps and policy_status |
| POST | `/portal/tunnels` | Create a tunnel |
| PUT | `/portal/tunnels/{id}` | Replace editable fields |
| DELETE | `/portal/tunnels/{id}` | Delete and release port/domain; no Self-hosted domain cooldown |
| POST | `/portal/bandwidth` | Atomically set `{allocations:[{tunnel_id,bps},...]}` |

Enrollment codes expire after 10 minutes and redeem only once. Long-lived device tokens authorize client APIs, not administrator login.

```json
{
  "name": "food-api",
  "client_id": "device-id",
  "protocol": "web",
  "local_host": "127.0.0.1",
  "local_port": 8000,
  "domain": "food-api.example.com",
  "enabled": true,
  "bandwidth_limit_bps": null
}
```

- Protocols: `tcp`, `udp`, `web`. Local hosts accept IPv4, IPv6, or localhost; ports are 1–65535.
- Web accepts full custom domains. A blank domain requires a configured suffix for random assignment. Domains use ASCII/Punycode, not URLs, paths, wildcards, or IP literals.
- TCP/UDP endpoints combine the host from `/me.public_addr` with the returned `public_port`; the Noise control port is not the service port.
- `public_port` may be allocated automatically or explicitly selected from the node pool. All protocols share the pool without duplicate allocation.
- `bandwidth_limit_bps`: `null` or omitted means unlimited, 0 disables, and positive integers are bit/s. `allocations[].bps` has the same semantics.
- Each rate limit aggregates both directions and all connections. There is no account allocation total. Increasing a limit or switching to unlimited does not automatically re-enable a disabled tunnel.

Successful save does not prove service reachability. `policy_status` indicates gateway policy application; synchronization pending/applied/error describes configuration reconciliation. Neither replaces a real application probe.

## Domains and local websites

`GET /domains` returns `{domain_suffix}`; `PUT /domains` accepts the same shape. An empty string clears the suffix. Full custom domains are not restricted to the suffix. Administrators still configure DNS.

Local websites require no client: `GET/POST /sites` lists/creates and `PUT/DELETE /sites/{id}` updates/deletes.

```json
{"name":"Blog","domain":"blog.example.com","upstream_host":"127.0.0.1","upstream_port":8080,"enabled":true}
```

Only loopback HTTP backends are allowed; localhost normalizes to 127.0.0.1. Disabling retains domain/port reservations. Deleting removes configuration, not the application. Website domains cannot overlap panel/tunnel domains; backend ports cannot use protected listeners. See [coexistence](COEXISTENCE.md).

## Nodes and operations

These endpoints require an administrator session; writes also require CSRF.

| Method | Path | Content |
|---|---|---|
| GET | `/dashboard` | Host CPU, memory, interface rates, heartbeat counts, synchronization state |
| GET / PUT | `/settings` | Node configuration; update caddy_url/public_addr/tls_ask_url |
| POST | `/settings/rotate-ask-key` | Rotate the Ask key and synchronize; never returns the key |
| POST | `/sync` | Reconcile immediately; failures return 502 and retain desired state |
| GET | `/diagnostics` | Version, schema, configuration and listener readiness |
| GET | `/audit` | Latest 100 events; up to 1000 retained |

Interface rates include other local applications and are not per-tunnel billing metrics. Background reconciliation runs every 3 seconds. Applied means configuration was written and Caddy accepted it, not that every tunnel is healthy.

## Client API, TLS, and health

Devices send `Authorization: Bearer <device-token>`:

- `GET /client/config`: client_id, enabled tunnels, complete rathole TOML, and revision. Protect this response because it contains service keys.
- `POST /client/heartbeat`: `{"running":true}`. The online window is 45 seconds; running describes process state only.

Both return 401 after revocation. Devices cannot manage other resources or node settings.

- `GET /api/v1/tls/check?domain=nas.example.com&key=<ask-key>`: returns 200 for the panel, enabled valid tunnels, or enabled local websites; otherwise 403.
- `GET /healthz`: unauthenticated liveness.
- `GET /readyz`: unauthenticated readiness. Requires successful synchronization within 15 seconds, matching revisions, and a reachable rathole control listener; otherwise 503.

## Compatibility

The core retains legacy `/clients`, `/tunnels`, and web assets for historical integration regression. Initialized Self-hosted rejects legacy resource writes with 409; new integrations use `/portal/*`.

Legacy Bearer admin tokens are not the current login flow. Explicit `HP_LEGACY_ADMIN=1` enables compatibility authentication on supported management endpoints without restoring legacy resource writes. SaaS registration, email, and user-management routes are not mounted.

## Administrator first-run setup

`GET /api/v1/setup` requires an administrator web session. It returns completion state, administrator email/display name, public control address, domain suffix, deployment panel domain and listener/port-range information. It never returns password hashes, bootstrap passwords or administration/Noise/certificate tokens. The `/me` response includes `setup_complete` for administrators in either edition; ordinary Cloud members do not receive this field.

`PUT /api/v1/setup` requires an administrator session and CSRF token. Body fields are `display_name`, `public_addr`, `domain_suffix`, and optional `current_password` / `password`. Validate all fields before committing profile, node address, suffix and completion state in one transaction. A password change checks the current password, invalidates existing sessions and returns `login_required: true`. Otherwise the current session remains valid. Configuration synchronization remains asynchronous (`status: pending`). Unknown fields are rejected.

Fresh databases start pending. Existing databases with stored node configuration are marked complete when upgrading, preserving their settings. The guide can be reopened at any time. Cloud does not expose these endpoints.
