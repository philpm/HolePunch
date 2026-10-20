# Development and testing

[English](DEVELOPMENT.md) | [简体中文](zh-CN/DEVELOPMENT.md)

## Toolchain and builds

Use Rust 1.91.1+, Python 3, and C/C++ build tools. Web assets are embedded with `include_str!`; rebuild the server after editing them. Server builds do not require Node. UI tests use Node 24, also required by the optional desktop app.

```sh
cargo build --workspace --locked
python3 scripts/check-repository.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/sync-i18n.py --check
node --test tests/i18n.test.cjs
```

Default workspace applications are `apps/selfhost` and `apps/cli`, producing `holepunch-server` and `holepunch-client`. Desktop has a separate workspace/lockfile; see its [guide](../apps/desktop/README.md). Cloud is not built here.

Use English for code comments, documentation, diagnostics, and examples. Keep GUI text in the [localization workflow](I18N.md). The generated catalog bundle is committed so normal Rust builds require no frontend tooling.

## Data-plane tools

Obtain Caddy 2.11.4 from its official distribution for your platform and place it at `output/tools/caddy` (`.exe` on Windows). rathole must include this project's patch and hot reload:

```sh
./scripts/prepare-sidecar.sh
```

This verifies the upstream rathole 0.5.0 archive, applies `scripts/patch-rathole.py` and a pinned lockfile, and builds server/client/noise/hot-reload support. Output is `apps/desktop/src-tauri/binaries/holepunch-rathole-<target>`. Copy it into `output/tools/rathole` for local development. For macOS ARM64:

```sh
mkdir -p output/tools
cp apps/desktop/src-tauri/binaries/holepunch-rathole-aarch64-apple-darwin output/tools/rathole
chmod +x output/tools/caddy output/tools/rathole
```

The sidecar path is shared with desktop packaging, but this script does not need Node/Tauri. `output/sidecar-source/` retains upstream build sources for release license inventories. The downstream `0.5.0-holepunch.2` version preserves wire framing and expands UDP receive buffers to 65,507 bytes.

## Local instance

```sh
python3 scripts/dev-local.py start
python3 scripts/dev-local.py stop
```

The panel is `http://localhost:13012`; random initial credentials are in `output/local-selfhost/ACCESS.md`. No environment setup is needed on first use. Stopping preserves data and bindings; the next start reuses them. `--edition selfhost` is a compatibility option and cannot select Cloud.

Ports: control plane 13012, Caddy Admin 12040, Noise 12354, HTTPS 18464, HTTP 18101, public pool 25100–25120. Gateways allocate additional internal loopback ports. Credentials, databases, certificates, process metadata, and logs stay in the ignored instance directory.

Enroll with the [CLI](CLI.md). Set an absolute `HP_RATHOLE_BIN` during development. The default client directory is `~/.holepunch`; use `--data-dir` to isolate tests.

## Real-link regression

```sh
python3 scripts/e2e-selfhost.py --caddy output/tools/caddy --rathole output/tools/rathole
python3 scripts/e2e.py --caddy output/tools/caddy --rathole output/tools/rathole
```

The first suite covers single-admin behavior, unlimited/optional limits, custom/random domains, and CLI enrollment. The second covers TCP/UDP/HTTPS, hot reload, revocation, backups/recovery, and local websites. See [testing evidence](TESTING.md).

Tests use isolated temporary directories, loopback ports, and an internal CA without changing system trust. Stop development instances that occupy internal ports first; Cloud can conflict and must be stopped from its own repository. Restore development instances afterward as needed. Test processes clean up their own children on exit.

Reports are written to `output/selfhost/latest.json` and `output/e2e/latest.json`; they point to full logs in temporary directories. Reports may contain local paths and raw directories may contain secrets. Redact before sharing.

## Interface checks

Check both languages on desktop and mobile layouts, keyboard navigation, open dialogs, form drafts, and user-entered Unicode text. Compatibility routes still embed `web/legacy`; update Rust paths when moving assets. Desktop status and build requirements are in its [guide](../apps/desktop/README.md).

Artifact generation, dependency review, and signing requirements are documented in [release](RELEASE.md). Existing workflows or old scan files do not prove current acceptance.

## Deployment tooling tests

```sh
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 tests/smoke-native.py --bin-dir target/debug --caddy output/tools/caddy --rathole output/tools/rathole
```

The unit suite uses temporary directories and never installs host services. The POSIX smoke test installs real binaries in an isolated directory, checks readiness and administrator login twice across a restart, and verifies listener cleanup on SIGTERM. It uses dynamically allocated ports and does not modify the normal local development instance. For cross-platform source sidecar builds, use `python3 scripts/build-rathole.py`; the Bash entry point delegates to it.
