# Web assets

[English](README.md) | [简体中文](../docs/zh-CN/WEB.md)

- `portal.html`, `portal.js`, `portal.css`: current session-authenticated console. Capabilities select available features; the server enforces permissions.
- `style.css`: base styles shared by current and compatibility pages.
- `legacy/index.html`, `legacy/app.js`: compatibility token console, still served at `/legacy` and `/app.js` and used by historical uninitialized mode. Self-hosted defaults to the portal.
- `i18n.js`, `locales/*.json`, `locales/catalogs.js`: shared locale runtime, source catalogs, and generated offline bundle. The optional desktop also consumes them.

The server embeds assets in `crates/server-core/src/lib.rs`. There is no frontend bundler and no Node requirement to build the server. After editing catalogs, run `python3 scripts/sync-i18n.py` and rebuild the server. See [localization](../docs/I18N.md).

Compatibility assets retain real routes and regression uses. Removing them requires a separate public API compatibility decision. Desktop-specific UI lives in `apps/desktop/ui`.
