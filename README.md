# HolePunch

Self-hosted TCP, UDP and HTTPS/WebSocket tunnels on your own server and domains.

**0.4.0-dev.1 development build** · [Documentation](docs/README.md) · [Apache-2.0](LICENSE) · [Cloud edition](https://github.com/philpm/HolePunch-Cloud)

One administrator workspace with email/password login, no public registration, and no product count quotas. Tunnels have unlimited bandwidth by default and optional per-tunnel aggregate bidirectional rate limits. There is no cumulative traffic accounting or monthly traffic cap; physical server and network limits still apply.

English is the default for documentation, development, and interfaces. The web and desktop interfaces support a persistent English/Simplified Chinese language selection.

Configure tunnels in the web console and connect each device with the shared CLI. Custom domains, random subdomains under your suffix, same-host websites, diagnostics and backups are supported. Signed installers and public images have not been released.

## Deploy and connect

Choose one of [three installation paths](docs/INSTALLATION.md):

- **Interactive native installer:** configure a verified release archive and optionally install a Linux systemd service. Portable foreground operation is available on macOS and Windows.
- **Clone and build:** run `python3 scripts/deploy.py install --source .`, then start the generated native runner. No Docker required.
- **Docker Compose:** build directly from this checkout, as below; published images are optional.

Release downloads require an actually published version; source builds and local archives work before publication. See [operations](docs/DEPLOYMENT.md) and [sharing an existing server](docs/COEXISTENCE.md).

```sh
python3 scripts/setup.py --public-host edge.example.com --panel-domain panel.example.com --admin-email admin@example.com
python3 scripts/compose.py up
```

Point DNS at the server first. The completion message lists the browser URL and initial email, password and administrator API token; a protected `.env.access.txt` copy is saved locally. Sign in to follow the four-step setup guide. Use the header’s **Language** selector to switch English/Chinese. The administrator API token is separate from the web password and client enrollment code. Use `--hide-secrets` on the startup command for automation logs.

Create a device in the console and run:

```sh
holepunch-client connect --server https://panel.example.com
```

Enter the one-time enrollment code when prompted. Keep the CLI running and manage all tunnel settings in the web console. See the [CLI manual](docs/CLI.md) for background services and troubleshooting.

## Develop

Rust 1.91.1+, Python 3 and C/C++ build tools are required. The web console is embedded in the server and needs no Node build.

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
cargo build --workspace --locked
cargo test --workspace --locked
# Prepare Caddy and patched rathole in output/tools first.
python3 scripts/dev-local.py start
```

The local panel is `http://localhost:13012`; credentials are in `output/local-selfhost/ACCESS.md`. See [development](docs/DEVELOPMENT.md), [architecture](docs/ARCHITECTURE.md), [API](docs/API.md) and [validation](docs/TESTING.md).

`apps/` contains the self-hosted server, shared CLI and optional desktop app. `crates/` contains the shared libraries; `web/`, `deploy/`, `scripts/` and `docs/` contain their supporting assets and documentation. Cloud is maintained in a separate Apache-2.0 repository and depends on a pinned public core revision. Desktop is maintained in this repository; its current GUI uses a long-lived device token rather than a one-time enrollment code.

[Localization](docs/I18N.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Changelog](CHANGELOG.md)

Apache-2.0. Third-party components retain their licenses; see [NOTICE](NOTICE.md).
