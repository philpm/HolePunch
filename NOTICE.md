# Third-party notices

- [rathole 0.5.0](https://github.com/rathole-org/rathole/tree/v0.5.0): Apache-2.0. Copyright Yujia Qiao and contributors. `scripts/patch-rathole.py` adjusts build metadata, terminal detection, dependencies, and supported features, and expands the UDP receive buffer from 2048 to 65507 bytes without changing wire framing. The downstream version is `0.5.0-holepunch.2`.
- [Caddy](https://github.com/caddyserver/caddy): Apache-2.0. Runs separately and is not linked into the HolePunch server binary.
- [Tauri](https://github.com/tauri-apps/tauri): MIT / Apache-2.0. Used by the optional desktop runtime and plugins.

The public workspace, desktop app, and rathole have separate Cargo lockfiles. Native server/CLI archives use Cargo metadata to generate core/client/rathole dependency and license inventories and collect available LICENSE/COPYING/NOTICE/COPYRIGHT files and build lockfiles. They do not include a desktop dependency inventory; desktop distribution requires its own review.

Inventories may include cross-platform records and do not prove every listed dependency is linked into a particular binary. Preserve applicable licenses and notices when distributing components. Signed installers and public images have not yet been released.
