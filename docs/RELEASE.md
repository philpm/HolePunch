# Self-hosted release guide

Core/CLI version: **0.4.0-dev.1**. Desktop version: **0.2.0-rc.2**, maintained independently. These are development builds; signed installers and public images have not been released. See [testing](TESTING.md) for evidence and the [roadmap](ROADMAP.md) for outstanding work.

## Native archives

Prepare tools and sidecar sources using the [development guide](DEVELOPMENT.md), then build:

```sh
cargo build --release --locked -p holepunch-server -p holepunch-cli
# The target must match the binaries; this example is macOS ARM64.
python3 scripts/package.py --target aarch64-apple-darwin --bin-dir target/release --rathole output/tools/rathole --out dist
```

Outputs are `holepunch-selfhost-<version>-<target>.tar.gz` (zip on Windows) and `.sha256`. Archives contain server, CLI, rathole, the portable installer/runner, shared `access.py` summary helper, standalone `setup.py`/`compose.py` launch tools and shell/PowerShell entry points, release Compose and Caddy bootstrap files, English documentation, systemd templates, license inventories, and build metadata. Caddy is downloaded and checksum-verified by the installer or supplied with `--caddy`; desktop is separate.

The archived server uses the compatibility installation name `bin/holepunch-center` but is built from `holepunch-server`. `BUILD.json` records edition, schema, public core revision, and version; do not infer the product from the executable name alone. Release from a clean committed checkout so recorded revision and content agree.

```sh
shasum -a 256 -c holepunch-selfhost-0.4.0-dev.1-aarch64-apple-darwin.tar.gz.sha256
tar -xzf holepunch-selfhost-0.4.0-dev.1-aarch64-apple-darwin.tar.gz
```

See [deployment](DEPLOYMENT.md) for installation, upgrades, backups, and rollback.

## Workflows and desktop

Manual `.github/workflows/release.yml` builds Linux x64/ARM64, Windows x64, macOS ARM64/x64 archives and Linux amd64 OCI image files. It uploads Actions artifacts; it does not automatically publish a GitHub Release or push images. Desktop installers require `include_desktop`, platform acceptance, and signing.

`deploy/compose.release.yml` requires a real published image prefix and pinned version. Do not point it at nonexistent images. Cloud has an independent open-source release process; public archives contain no SaaS implementation.

Locale runtime and catalogs are embedded in the server. Desktop copies the same assets during its Rust build. No runtime translation service or network download is required. Run `python3 scripts/sync-i18n.py --check` before packaging to reject stale bundles.

## Making downloads available

After acceptance and authorization to publish, create a GitHub Release on a tag exactly matching `v<Cargo version>`. Upload the native archives under their generated filenames and each adjacent `.sha256` file. The installer selects the platform target and expects those names. Publish from a clean checkout; a locally generated archive with uncommitted changes is only a test artifact.

The bootstrap scripts download `scripts/deploy.py` and `scripts/access.py` from that same tag. Keep the scripts accessible in the tagged source. Test a clean-host install through the actual public URL before announcing one-command installation. Until assets exist, document the source or local-archive path.

For image distribution, push the tested center and rathole images under the same registry prefix and version tag, then distribute `compose.release.yml` with `bootstrap.Caddyfile`. Verify fresh bootstrap login with the image-based Compose stack, not just the source stack. Artifact-building workflows deliberately do not make the repository public or publish releases/images automatically.

## Dependency review

```sh
python3 scripts/audit-dependencies.py
# Keep informational advisories as REVIEW; ordinary vulnerabilities still block.
python3 scripts/audit-dependencies.py --allow-informational
```

The tool scans current public, desktop, and rathole lockfiles and writes ignored `output/dependency-audit-current.json`. Obsolete snapshots are not current evidence. Inventories do not prove exploitability or reachability in a particular binary. Rescan before release and resolve blockers and review notices.

## Release requirements

- Public workspace tests, formatting, Clippy, and both real-link regression suites pass.
- Localization parity/runtime checks pass; verify English and Chinese on actual web/desktop flows and ensure translated output does not replace user data.
- Verify target-platform native deployment, Linux Docker, upgrade/rollback, public DNS/CA, and sustained load on real systems.
- Ensure archive checksums, versions, dependency inventories, and licenses match actual binaries.
- Independently verify desktop credentials, autostart, quit behavior, signing/notarization; server acceptance does not cover these.
- Review source and Git history before publication and establish private vulnerability reporting. Exclude runtime credentials, databases, and private operations material from artifacts.

A workflow file does not prove remote CI ran. Promote release status only after the corresponding checks finish.
