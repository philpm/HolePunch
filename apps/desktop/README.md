# Optional desktop client

[English](README.md) | [简体中文](../../docs/zh-CN/DESKTOP.md)

The Tauri/Rust app reuses `crates/client-core` and the same device protocol as Self-hosted and Cloud. Server and CLI builds do not require Node, Tauri, or desktop system dependencies.

Desktop has its own Cargo workspace, version, and lockfile. Its current binding form accepts a long-lived device token. One-time enrollment UI is planned; use the CLI for the primary enrollment flow.

```sh
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked
./scripts/prepare-sidecar.sh
npm ci --prefix apps/desktop
npm run dev --prefix apps/desktop
# Build an installer:
npm run build --prefix apps/desktop
```

Use Node 24 and the Tauri platform dependencies: Xcode Command Line Tools on macOS, MSVC/WebView2 on Windows, and WebKitGTK/AppIndicator/Secret Service on Linux. `HP_RATHOLE_BIN` overrides the sidecar path at runtime.

English is the default. The language selector persists English or Simplified Chinese locally and also updates tray labels. `build.rs` copies the shared offline runtime and catalog bundle from `web/` into the packaged UI. Edit the source catalogs, not generated desktop copies; see [localization](../../docs/I18N.md).

Closing the window keeps the app in the tray. Quitting terminates the client child process. Autostart requires explicit opt-in; restoring a binding does not automatically start tunnels. Distribution still requires target-platform verification, signing, and macOS notarization.

Release workflows build server/CLI archives by default and desktop installers only with `include_desktop`. Desktop is maintained in this Apache-2.0 repository; see [EDITIONS](../../docs/EDITIONS.md).

## Brand assets

The [master logo](../../docs/assets/logo.png) represents a connection passing through an open portal. Its lime mark and charcoal background are shared by both products, the browser console, and the desktop app. The master PNG has transparent outer corners and is distributed under the repository's Apache-2.0 license.

After replacing the master, regenerate the committed PNG, ICO, and ICNS assets with the installed Tauri CLI:

```sh
npm ci --prefix apps/desktop
python3 scripts/sync-icons.py
```

The script keeps only the required desktop and browser assets. The desktop build copies `web/logo.png` into its UI automatically. Rebuild the server and desktop app to embed updated icons. Copy the same master into the Cloud repository's `docs/assets/logo.png` when updating its core submodule.
