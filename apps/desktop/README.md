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

The [master logo](../../docs/assets/logo.png) represents two network nodes connected through a circular barrier, with diagonal strokes suggesting traversal. Its olive-green mark and white background are shared by both products, the browser console, and the desktop app. The master is a 1024 × 1024 PNG with an opaque square background, distributed under the repository's Apache-2.0 license.

Transparent variants use a geometric SVG redraw with concentric circles, circular arcs, and rounded strokes. The PNG exports match this vector geometry. All brand assets are covered by the same Apache-2.0 license.

| Asset | Use |
| --- | --- |
| [Olive SVG](../../docs/assets/logo.svg) | Scalable mark on light backgrounds; editable vector source |
| [Transparent olive PNG](../../docs/assets/logo-transparent.png) | 1024 × 1024 raster export with true alpha transparency |
| [White SVG](../../docs/assets/logo-light.svg) | Scalable mark on dark or colored backgrounds |
| [Transparent white PNG](../../docs/assets/logo-light.png) | 1024 × 1024 white raster export with true alpha transparency |

SVGs contain native geometry with a `0 0 1024 1024` viewBox and no background, embedded bitmap, font, or external dependency. Preserve the square aspect ratio and clear space. For a custom color, change the root `color` attribute: shapes use `currentColor`. Inline SVG also accepts a CSS `color`; SVG loaded through `<img>` uses its own root color rather than the surrounding page's color. Use the white variant where olive has insufficient contrast.

After replacing the opaque master or editing `docs/assets/logo.svg`, regenerate the committed PNG, ICO, ICNS, and transparent variants with the installed Tauri CLI:

```sh
npm ci --prefix apps/desktop
python3 scripts/sync-icons.py
```

The script generates platform icons from `logo.png`, and the transparent PNGs and white SVG from `logo.svg`. The desktop build copies `web/logo.png` into its UI automatically. Rebuild the server and desktop app to embed updated icons. Copy all five logo assets into the Cloud repository's `docs/assets/` when updating its core submodule.
