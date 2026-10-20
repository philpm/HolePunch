# Testing

[English](TESTING.md) | [简体中文](zh-CN/TESTING.md)

Use the commands and tool preparation steps in [Development](DEVELOPMENT.md). Run checks against the commit being reviewed; old acceptance reports are not evidence for a new release.

| Area | Required checks |
|---|---|
| Rust workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo build --workspace --locked` |
| Repository and documentation | `python3 scripts/check-repository.py` |
| Deployment tools | `python3 -m unittest discover -s tests -p 'test_*.py' -v` |
| Localization | `python3 scripts/sync-i18n.py --check`, `node --test tests/i18n.test.cjs` |
| Native installer | `python3 tests/smoke-native.py --bin-dir target/debug --caddy output/tools/caddy --rathole output/tools/rathole` |
| Self-hosted data plane | `python3 scripts/e2e-selfhost.py --caddy output/tools/caddy --rathole output/tools/rathole` |
| Shared recovery and forwarding | `python3 scripts/e2e.py --caddy output/tools/caddy --rathole output/tools/rathole` |

Integration suites use isolated temporary directories and real subprocesses. Prepare checksum-pinned Caddy and patched rathole first; free conflicting ports without stopping unrelated services. They cover TCP/UDP/HTTPS, bandwidth limits, revocation, reconnection, local websites, and backup/recovery. These checks do not establish public DNS, SMTP, or platform support.

## Interface acceptance

Check English default, manual Simplified Chinese selection, persistence after reload, keyboard navigation, accessible labels, mobile overflow, and light/dark themes. Switch language with dirty forms and open dialogs. Preserve user-entered names, domains, HTML metacharacters, placeholder braces, and Unicode. Check known API errors and raw diagnostic fallback. See [Localization](I18N.md).

Desktop has its own Cargo workspace. Run its build/check steps in the [desktop guide](../apps/desktop/README.md) and verify native tray, credential storage, and process lifecycle on each target platform. A mocked browser bridge cannot establish native behavior.

## Evidence and release acceptance

Keep raw reports, screenshots, credentials, and temporary logs under ignored `output/` or outside the checkout. Public pull requests should include only redacted results, the tested commit, platform, commands, and limitations. Never commit a production acceptance diary or customer-specific deployment details.

CI runs portable deployment checks, Rust checks, localization, Compose builds, and Linux integration suites. Release acceptance additionally needs real target-platform installs, upgrades/rollback, public TLS, sustained load, and dependency/license review. A workflow definition does not prove a run passed. See [Release](RELEASE.md).
