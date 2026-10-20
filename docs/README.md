# Documentation

[English](README.md) | [简体中文](zh-CN/README.md)

These guides describe the current Self-hosted development build. English is authoritative. The guides describe supported behavior and repeatable validation; internal acceptance diaries are excluded.

| Task | Guide |
|---|---|
| Understand the product | [Product overview](OVERVIEW.md) |
| Choose installer, source build or Compose | [Installation](INSTALLATION.md) |
| Deploy, upgrade, back up, or troubleshoot | [Deployment](DEPLOYMENT.md) |
| Share a VPS with existing web/PHP sites | [Coexistence](COEXISTENCE.md) |
| Connect a device or run it in the background | [CLI](CLI.md) |
| Build and test locally | [Development](DEVELOPMENT.md) |
| Understand the code and data plane | [Architecture](ARCHITECTURE.md) |
| Call the current API | [API](API.md) |
| Review validation evidence and limitations | [Testing](TESTING.md) |
| Build artifacts and check release readiness | [Release](RELEASE.md) |
| See planned work | [Roadmap](ROADMAP.md) |
| Understand public/Cloud/desktop ownership | [Repository boundaries](EDITIONS.md) |
| Add or maintain a UI language | [Localization](I18N.md) |

[Contributing](../CONTRIBUTING.md) · [Support](../SUPPORT.md) · [Code of conduct](../CODE_OF_CONDUCT.md) · [Security](../SECURITY.md) · [Maintaining](MAINTAINING.md) · [Third-party notices](../NOTICE.md)

## Local generated directories

`target/`, desktop `target/`, and `node_modules/` are rebuildable caches. `dist/` holds generated release artifacts. None are committed.

`output/tools/` contains local Caddy/rathole binaries. `output/local-selfhost/` contains the development instance's credentials, database, and process metadata. `output/selfhost/`, `output/e2e/`, and `output/playwright/` contain test evidence. Sidecar source directories support builds and license manifests. All are ignored by Git; runtime data and evidence may contain secrets and are not public issue attachments.

Run `python3 scripts/dev-local.py stop` and back up data before cleaning an active instance. Do not treat all of `output/` as disposable build cache.
