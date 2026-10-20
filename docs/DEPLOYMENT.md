# Deployment and operations

Start with [the three installation paths](INSTALLATION.md): interactive native installation, clone/build/run, or Docker Compose. This guide covers runtime configuration and operations. The installer-managed systemd unit is `holepunch.service`; the three separate units below are advanced manual templates. Use one process-management approach per installation.

This repository builds Self-hosted and the shared CLI with database schema 4. Cloud lives in a separate Apache-2.0 repository; see [boundaries](EDITIONS.md). On first start, initialize one administrator through `HP_BOOTSTRAP_EMAIL` and `HP_BOOTSTRAP_PASSWORD`. Self-hosted has no registration or SMTP service. Back up existing data before upgrading; a Cloud data directory cannot be opened directly.

The Compose example below uses dedicated Linux listeners. Docker is optional. For native deployment or an existing Nginx/BT Panel host, also read [coexistence](COEXISTENCE.md). Never publish managed tunnels' internal loopback ports.

## Prerequisites

- Linux VPS with Docker Engine and Compose v2, and enough disk space for a Rust source build.
- Public IP and DNS A/AAAA records for the panel and web tunnels. Omit AAAA without working IPv6.
- Reachable TCP 80, 443, 2333, and required tunnel ports. UDP tunnels need the corresponding UDP firewall rules.
- No conflicting listeners on 80, 443, 2019, 2333, or 3000 for the default configuration.

Compose uses `network_mode: host` so Caddy can reach loopback-only rathole web ports. The deployment target is Linux. Docker Desktop host networking has separate settings and limitations and is not the acceptance environment for this Compose stack.

## First start

```sh
python3 scripts/setup.py --public-host edge.example.com --panel-domain panel.example.com
python3 scripts/compose.py up
docker compose ps
docker compose logs -f center rathole
```

Replace example domains with your own. Setup refuses to overwrite an existing `.env` and creates it with mode 0600. `HP_PUBLIC_ADDR` is the rathole connection address, not the panel URL. After initial database creation, the public address, Caddy API, and Ask URL are database settings. Edit them in the console; changing their initial environment values does not overwrite the database.

Center generates `server.toml`; Caddy starts with a loopback-only Admin bootstrap, then receives managed routes. rathole waits for Center health and reads the shared configuration directory. Compose restart retries if initial configuration is not yet available.

Caddy issues a certificate for the explicitly configured panel domain. First access may wait for issuance. The startup wrapper waits for readiness and prints browser access, initial credentials and the private summary path. Fresh installations open the setup guide after sign-in. Sign in using the generated `HP_BOOTSTRAP_EMAIL` and `HP_BOOTSTRAP_PASSWORD` from `.env`. Bootstrap variables do not reset an existing password. Do not paste credentials into issues or logs.

Without a panel domain:

```sh
python3 scripts/setup.py --public-host 203.0.113.10
python3 scripts/compose.py up
# Run on your own computer; replace the server address.
ssh -L 3000:127.0.0.1:3000 user@203.0.113.10
```

Open `http://127.0.0.1:3000` locally. Clients can also use this SSH-forwarded URL, but configure a public HTTPS panel for ongoing use.

## First tunnel

1. Sign in with the initial administrator credentials and create a device in **Clients**.
2. Run the displayed CLI command, enter its one-time enrollment code, and keep the CLI running.
3. In **Tunnels**, choose the device, protocol, local address, and port. Blank bandwidth is unlimited; 0 disables the tunnel.
4. For Web, enter your full domain and point DNS at the edge server. Alternatively, configure a suffix in **Domain settings** for random assignment.
5. Check configuration state, heartbeat, and a real application request. Desktop is optional; its one-time enrollment flow is not implemented yet.

The console starts in English. Its language selector switches to Simplified Chinese and remembers the choice in this browser. CLI and server diagnostics remain English.

## Environment variables

| Variable | Default or purpose |
|---|---|
| `HP_BOOTSTRAP_EMAIL` / `HP_BOOTSTRAP_PASSWORD` | Required initial administrator login; password at least 12 bytes |
| `HP_ADMIN_TOKEN` | Required, at least 32 bytes; 64 random hex characters recommended |
| `HP_DATA_DIR` | Local `data`; image `/data` |
| `HP_BIND` | `127.0.0.1:3000`, Center HTTP listener |
| `HP_PUBLIC_ADDR` | Initial `127.0.0.1:2333`; update in the console afterward |
| `HP_RATHOLE_BIND` | `0.0.0.0:2333` |
| `HP_CADDY_URL` | Initial `http://127.0.0.1:2019` |
| `HP_TLS_ASK_URL` | Initial `http://127.0.0.1:3000/api/v1/tls/check` |
| `HP_PANEL_DOMAIN` | Optional panel FQDN, reserved against tunnel use |
| `HP_PORT_START` / `HP_PORT_END` | `20000` / `20100`, inclusive range within 1024–65535 |
| `HP_CADDY_LISTEN` | `127.0.0.1:2019`, Admin listener in managed configuration |
| `HP_HTTPS_LISTEN` / `HP_HTTP_PORT` | `:443` / `80`; overridable for local tests |
| `HP_TLS_INTERNAL` | Only `1` enables the local test CA; automatic trust installation is disabled. Do not enable for public production |
| `RUST_LOG` | `info` |

Changing the Center port also requires changing the Ask URL. The panel proxy follows `HP_BIND`, converting wildcard listeners to loopback. Changing the Caddy Admin port requires matching updates to its bootstrap, `HP_CADDY_LISTEN`, and the saved Caddy URL.

## Full backup and restore

For a simple consistent backup of the database, WAL, Noise keys, and certificates, stop the stack first:

```sh
docker compose stop
# Actual volume names include the Compose project prefix.
docker volume ls
```

Back up the actual `holepunch-data`, `caddy-data`, and `caddy-config` volumes and protected `.env`. A temporary container can mount and archive them. Backups contain service keys and must be encrypted. Do not copy only a live `.db` and omit its WAL.

Restore into empty volumes with the same image version, retain UID/GID 10001 ownership for application data, and start Compose. Center resets client heartbeats and reapplies database configuration at startup.

`docker compose down -v` deletes persistent configuration and certificates; use it only when intentionally removing all data.

## Upgrade and rollback

1. Stop services and back up.
2. Update source and lockfiles, then run `docker compose build`.
3. Run `docker compose up -d`, inspect logs, and test real tunnel traffic.
4. To roll back, restore the matching version's data backup as well as its binaries. Schema 4 migrates supported historical single-admin data; Cloud directories do not convert directly. Automatic schema downgrade is unsupported.

## Online database backup

```sh
HP_DATA_DIR=data target/release/holepunch-server backup /secure/backups/holepunch.db
# Stop Center first; restore refuses while it is running.
HP_DATA_DIR=data target/release/holepunch-server restore /secure/backups/holepunch.db
```

Backup uses SQLite Online Backup and includes committed WAL data; the destination must not already exist. Restore checks integrity/schema, takes an exclusive directory lock, and preserves the previous database as `before-restore-*.db`. Encrypt backups separately. Environment files and Caddy certificate volumes need separate backup.

`/healthz` indicates process liveness. `/readyz` also requires successful configuration synchronization within 15 seconds, matching revisions, and a reachable rathole control listener. Compose startup dependencies use liveness to avoid a rathole/readiness cycle; external monitors should use readiness.

## Native systemd deployment

Native archives include systemd templates. Create a `holepunch` system user without a login shell, install binaries in `/usr/local/bin`, create `/etc/holepunch/center.env` or `client.env` with mode 0600, and install the matching units. Public entry also needs a separate Caddy 2.11.4 service bootstrapped with `deploy/caddy.json` (archive root `caddy.json`). Do not run Compose and native units for the same stack simultaneously.

For `deploy/systemd/holepunch-caddy.service`, install verified Caddy at `/usr/local/bin/caddy`, create a non-login `hpcaddy` user and its `/var/lib/holepunch-caddy` directory (0700), then copy the bootstrap to `/etc/holepunch-caddy/bootstrap.json` (directory 0755, file 0644).

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now holepunch-caddy holepunch-center rathole
curl -f http://127.0.0.1:3000/readyz
```

Do not also start the distribution's `caddy.service`. The dedicated unit uses `CAP_NET_BIND_SERVICE` to bind 80/443 without root and persists certificates in its own data directory. Include that directory and `/etc/holepunch/center.env` in backups.

`deploy/compose.release.yml` uses the adjacent `bootstrap.Caddyfile` and is for real published images and requires an actual `HP_IMAGE_PREFIX` and pinned `HP_VERSION`. No fictional public image registry is configured. Compose services use read-only root filesystems and bounded container logs.

## Troubleshooting

| Symptom | Check |
|---|---|
| Synchronization fails | Caddy Admin reachability, writable configuration directory, port 2019 conflicts; reapply and inspect logs |
| Applied configuration, unreachable TCP | rathole hot reload, occupied ports, firewall rules, client running |
| Online client, unreachable application | Heartbeat proves API reachability only; inspect `rathole.log` and the local service |
| UDP fails | Both provider and OS firewalls need UDP rules; a TCP rule is insufficient |
| HTTPS 502 | Matching rathole web listener and local HTTP application; Web currently proxies plaintext HTTP upstream |
| Certificate fails | DNS/IPv6, 80/443, clock, CA rate limits, and an enabled domain on a valid client |
| Manual Caddy changes disappear | This instance is managed; reconciliation repairs drift |
| Port creation fails | Pool exhaustion/conflict; an external OS listener may be reported only in rathole logs |
| Desktop cannot find rathole | Run prepare-sidecar or set absolute `HP_RATHOLE_BIN` |
| Credential-store save fails | Check Keychain/Credential Manager/Secret Service, or disable remembering the binding |

Synchronization failure does not undo saved edits. Fix the node and automatic retry converges to the latest desired state.

Provider firewalls and host UFW are separate layers. If the service listens and UFW allows it but no external SYN arrives, check the provider's security group or upstream network. A CDN proxy can front the management panel; the rathole public address needs the VPS IP or a separate hostname that bypasses ordinary HTTP CDN proxying.

To share 80/443 with other applications, register them under **Local websites** and keep their HTTP listeners on loopback. Read [coexistence](COEXISTENCE.md); do not append manual sites to the managed Caddy instance.
