# HolePunch

<img src="docs/assets/logo.svg" alt="HolePunch logo" width="96">

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Rust 1.91.1+](https://img.shields.io/badge/Rust-1.91.1%2B-orange.svg)

[English](README.md) | [简体中文](README-zh.md)

A self-hosted tunnel service with a web console, written in Rust.

HolePunch exposes services on your NAS, development machine, or other devices behind NAT through a server with a public IP. It uses [rathole](https://github.com/rathole-org/rathole) for device connections and [Caddy](https://github.com/caddyserver/caddy) for HTTPS. Enroll a device with the CLI, then manage its tunnels in the browser.

> **Development version: 0.4.0-dev.1.** Source installation is available. Release downloads and prebuilt images require an actually published version; signed desktop installers are not yet released.

- [Features](#features)
- [Quickstart](#quickstart)
- [Configuration](#configuration)
- [Documentation](#documentation)
- [Build from source](#build-from-source)
- [Planning](#planning)
- [Contributing](#contributing)
- [License](#license)

## Features

- **TCP, UDP, and Web tunnels.** Publish local services through public ports or HTTPS domains, including WebSocket applications.
- **Browser-based configuration.** Create devices and tunnels, manage domains, inspect status, and run diagnostics from one console.
- **One-time enrollment.** Connect a device with a short-lived code. The CLI saves its scoped credentials and synchronizes tunnel settings automatically.
- **Your server, your domains.** Use custom domains, random subdomains under your suffix, or serve existing applications on the same VPS.
- **Optional bandwidth limits.** Tunnels are unlimited by default. A finite limit is shared across both directions and all connections; there are no product count quotas or cumulative traffic caps.
- **English and Simplified Chinese.** Switch the web or optional desktop interface language without losing form drafts.
- **Native or container deployment.** Use a native installer, build from source, or run Docker Compose. SQLite, the embedded console, and a single administrator keep the Self-hosted edition self-contained.

For registration, email invitations, multiple users, and per-user allowances, see [HolePunch Cloud](https://github.com/philpm/HolePunch-Cloud). Both projects use Apache-2.0 and share the same CLI.

## Quickstart

You need a server with a public IP and a device running the local service. The example below publishes a NAS web application at `127.0.0.1:8080` as `https://nas.example.com`.

### 1. Start the server

On a Linux server with Git, Python 3.10+, Docker Engine, and Compose v2:

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
python3 scripts/setup.py --public-host edge.example.com \
  --panel-domain panel.example.com --admin-email admin@example.com
python3 scripts/compose.py up
```

Replace the example domains with your own. Point `edge.example.com`, `panel.example.com`, and `nas.example.com` to the server. Allow TCP 80/443 for HTTPS and TCP 2333 for device connections; TCP/UDP tunnels also need their selected public ports. Use a DNS-only hostname for `edge.example.com`.

The command prints the panel URL and initial administrator credentials after the stack is ready. Open `https://panel.example.com`, sign in, and follow **Setup guide**. Protect `.env` and `.env.access.txt`; use `--hide-secrets` when capturing startup logs.

For installation without Docker, or a server already hosting websites, read [Installation](docs/INSTALLATION.md) and [Coexistence](docs/COEXISTENCE.md).

### 2. Connect your device

Create a device in **Clients**. On the NAS, use a matching native package and keep `holepunch-client` and `rathole` together in its `bin/` directory. Add that directory to `PATH`, then run:

```sh
holepunch-client connect --server https://panel.example.com
```

Enter the one-time enrollment code when prompted. Keep the client running. If release packages are unavailable, build the CLI and patched sidecar using the [development guide](docs/DEVELOPMENT.md). The [CLI manual](docs/CLI.md) explains background services and credential storage.

### 3. Create a tunnel

In **Tunnels**, choose the device and create a Web tunnel:

| Field | Value |
|---|---|
| Name | `nas-web` |
| Protocol | Web |
| Local address | `127.0.0.1` |
| Local port | `8080` |
| Domain | `nas.example.com` |
| Bandwidth | Leave blank for unlimited |

Save, wait for configuration synchronization, then open `https://nas.example.com`. Keep the local application running. For TCP or UDP, use the public host and port shown in the console.

## Configuration

Manage devices, tunnels, domains, and optional rate limits in the console. The server generates rathole and Caddy configuration; manual edits to generated files are overwritten.

Deployment settings such as listeners, data directories, and the panel domain live in `.env` or the native installation's protected `config.json`. See the [environment variable reference](docs/DEPLOYMENT.md#environment-variables). Port changes still require matching DNS, firewall, and edge routing configuration.

### Logging and troubleshooting

```sh
docker compose logs -f center rathole caddy
holepunch-client status
holepunch-client doctor
holepunch-client logs --follow
```

`/healthz` checks liveness; `/readyz` checks local control-plane readiness. Test the actual tunneled service as well. Native systemd logs, backups, upgrades, and rollback are covered in [Deployment](docs/DEPLOYMENT.md).

## Documentation

| Guide | What it covers |
|---|---|
| [Installation](docs/INSTALLATION.md) | Native installer, source build, and Docker Compose |
| [Deployment](docs/DEPLOYMENT.md) | Configuration, backups, upgrades, and troubleshooting |
| [CLI](docs/CLI.md) | Enrollment, background services, and diagnostics |
| [Architecture](docs/ARCHITECTURE.md) | Shared core, forwarding, and repository layout |
| [API](docs/API.md) | Sessions, devices, tunnels, setup, and health endpoints |
| [Localization](docs/I18N.md) | UI catalogs and translated documentation |

[All documentation](docs/README.md) · [中文文档](docs/zh-CN/README.md) · [Security](SECURITY.md)

## Build from source

Use Rust 1.91.1+, Python 3.10+, and native C/C++ build tools. Node.js 24 is needed for UI tests and the optional desktop app; server builds embed the web assets directly.

```sh
cargo build --workspace --locked
cargo test --workspace --locked
```

Prepare Caddy and the patched rathole as described in [Development](docs/DEVELOPMENT.md), then run `python3 scripts/dev-local.py start`. The local panel is `http://localhost:13012`; generated credentials stay in `output/local-selfhost/ACCESS.md`.

## Planning

Current work focuses on release acceptance across target platforms, desktop one-time enrollment, and diagnostic export. Remote nodes and automatic route selection are future work. See the [roadmap](docs/ROADMAP.md) and [testing requirements](docs/TESTING.md); no comparative performance claims are made without project-specific measurements.

## Contributing

Bug reports, focused pull requests, and translations are welcome. Read [Contributing](CONTRIBUTING.md), [Support](SUPPORT.md), and the [Code of conduct](CODE_OF_CONDUCT.md). Report vulnerabilities through the private process in [Security](SECURITY.md).

## License

HolePunch is licensed under [Apache-2.0](LICENSE). Third-party components retain their own licenses; see [NOTICE](NOTICE.md). Thanks to the rathole, Caddy, and Tauri communities for the foundations this project builds on.
