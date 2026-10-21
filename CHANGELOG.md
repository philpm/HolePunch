# Changelog

[English](CHANGELOG.md) | [简体中文](docs/zh-CN/CHANGELOG.md)

## 0.4.0-dev.1 — First public pre-release

- Publish portable server/CLI archives for Linux x64/ARM64, macOS x64/ARM64, and Windows x64 with SHA256 checksums.
- Verify the packaged installer, binary versions, login, restart, retained credentials, and shutdown on each target platform.
- Record both product and shared-core source revisions in release metadata.
- This development pre-release excludes signed desktop installers and published container images.

- Handle Windows console-break shutdown in the native supervisor.
- Fix Windows CLI log rotation and database backup synchronization; package license files with timestamps older than the ZIP format supports.

### CI and dependency maintenance

- Fix translation revision checks for Windows CRLF files.
- Update TLS/QUIC dependencies to patched Rustls 0.23.46, Quinn-proto 0.11.18, Rustls-webpki 0.103.16, and Webpki-roots 1.0.10.
- Update cryptography and TOML dependencies while preserving existing password hashes, token encodings, and rathole configuration compatibility.
- Update desktop tooling and GitHub Actions; group compatible dependency PRs, avoid duplicate CI runs, and retain the tested Rust baseline.
- Allow the shared dependency audit to check a downstream repository and explicit lockfiles.

### bilingual documentation

- Rewrite both project homepages around features, a complete quickstart, configuration, and contributor resources.
- Add Simplified Chinese homepages and matching user, developer, and community guides with source revision checks.
- Include translated documentation and project artwork in native archives.

### open-source maintenance

- Adopt Apache-2.0 across both products and package metadata.
- Publish contributor, support, conduct, localization, and maintenance guides.
- Consolidate product documentation and remove internal acceptance diaries from public history.
- Check repository hygiene and documentation links in CI; retain English/Simplified Chinese validation.

### first-run experience

- Make language switching visible in both sign-in and console headers.
- Add an authenticated four-step administrator setup guide shared by both editions with persistent completion, atomic configuration updates and optional administrator password replacement.
- Show deployment access URLs and initial credentials after native installation or Compose readiness; save protected summaries and support `--hide-secrets` for automation.
- Explain client enrollment separately from administrator API tokens, public DNS/firewall requirements and existing-site coexistence.

### deployment tooling

- Add an interactive native installer with source/archive/release inputs, private configuration, checksum verification, port conflict checks and a shared process supervisor.
- Add Linux systemd installation, shell/PowerShell entry points, portable Python sidecar builds and Linux ARM64 release artifact configuration.
- Document native installation, clone/build/run and Linux Docker Compose as three distinct deployment paths.
- Fix administrator bootstrap credentials in release Compose and support consistent custom listener ports in both Compose variants.
- Add portable deployment regression tests and a real native lifecycle smoke; container images remain pending publication.


### Self-hosted features

- Separated the public core, Self-hosted server and shared CLI from the private Cloud repository; desktop remains optional.
- Added administrator password sessions, single-use CLI enrollment, own domains, random subdomains and optional per-tunnel bandwidth limits; no traffic accounting.
- Added schema 4 unlimited policies, edition-pinned data directories, server-owned gateways and recovery tests.
- Reorganized the web console into sidebar pages with mobile navigation and form draft protection.
- Expanded the patched rathole UDP receive buffer without changing wire framing.
- Consolidated current documentation and isolated compatibility web assets; historical drafts and audit snapshots were removed from the working tree.
- Established English-first documentation, comments, CLI/API diagnostics, and shared English/Simplified Chinese web/desktop localization, with persistent selection and catalog validation.
- This is a development version, not a public stable release.

## 0.2.0-rc.2 — same-host websites

- Added persistent same-host website management, loopback HTTP proxying and automatic TLS permissions.
- Shared domain/port conflict checks across websites, tunnels and panel; websites survive client revocation and edge restarts.
- Added schema 2 migration, backup/restore coverage, browser management and real HTTPS coexistence tests.
- Added a coexistence guide for native, Docker and PHP applications.

## 0.2.0-rc.1 — community release candidate

- Recover automatically from transient control-plane outages; pause after a 90-second authorization lease and resume after a successful sync. Explicit revocation stops the process and removes cached service credentials.
- Bound client logs to approximately 4 MiB, add in-app diagnostics and local unbinding.
- Add schema version checks, atomic desired-state/audit transactions, WAL-aware online backups, locked restore with preservation of previous data, and a single-server data-directory lock.
- Separate process liveness from edge readiness, add diagnostic and recent-operation APIs, support SIGTERM shutdown, and cap Docker log retention.
- Add portable archive packaging, checksums, release-candidate CI artifacts, OCI image build exports, systemd units and a pinned-image deployment template.
- This is a candidate, not an announcement of signed or publicly published binaries. See the release acceptance matrix.

## 0.1.0 — initial MVP

Axum control plane, SQLite configuration, TCP/UDP/Web tunnels, Caddy On-Demand TLS, rathole Noise transport, Tauri desktop client and CLI, and initial integration tests.
