# 可选桌面客户端

[English](../../apps/desktop/README.md) | 简体中文 · [文档目录](README.md)

Tauri/Rust 应用复用 `crates/client-core`，与 Self-hosted、Cloud 使用同一设备协议。服务端和 CLI 构建不依赖 Node、Tauri 或桌面系统依赖。

桌面有独立 Cargo 工作区、版本和锁文件。当前绑定表单接受长期设备令牌，一次性接入 UI 仍在计划中，主要接入流程请使用 CLI。

```sh
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --locked
./scripts/prepare-sidecar.sh
npm ci --prefix apps/desktop
npm run dev --prefix apps/desktop
# Build an installer:
npm run build --prefix apps/desktop
```

使用 Node.js 24 和 Tauri 平台依赖：macOS 为 Xcode Command Line Tools，Windows 为 MSVC/WebView2，Linux 为 WebKitGTK/AppIndicator/Secret Service。运行时可用 `HP_RATHOLE_BIN` 覆盖 sidecar 路径。

默认英文，语言选择器本地保存英文/简体中文偏好并更新托盘。`build.rs` 将 `web/` 中共享离线运行时和词条包复制进 UI。只编辑源目录，不编辑生成的桌面副本，见[国际化](I18N.md)。

关闭窗口后留在托盘，退出应用会终止客户端子进程。自动启动需显式选择，恢复绑定不会自动启动隧道。分发仍需目标平台验证、签名及 macOS 公证。

发布工作流默认构建服务端/CLI 归档，只有 `include_desktop` 才构建桌面安装器。桌面维护在本 Apache-2.0 仓库，见[版本边界](EDITIONS.md)。

## 品牌资源

[标志源文件](../assets/logo.png)表达“连接穿过孔洞”的含义。荧光绿图形和深灰底色由两个项目、浏览器控制台与桌面应用共同使用。源 PNG 的外侧圆角区域透明，按仓库的 Apache-2.0 协议分发。

替换源文件后，使用已安装的 Tauri CLI 重新生成需要提交的 PNG、ICO 和 ICNS 资源：

```sh
npm ci --prefix apps/desktop
python3 scripts/sync-icons.py
```

脚本仅保留桌面端和浏览器需要的资源。桌面构建会自动将 `web/logo.png` 复制到界面目录。重新构建服务端与桌面应用即可嵌入更新后的图标。更新 Cloud 的核心子模块时，也应将同一份源文件复制到 Cloud 仓库的 `docs/assets/logo.png`。

<!-- translation-source: apps/desktop/README.md; source-sha256: e629a111207731eb409b29ccdec82ccc3956a8b0a34d3fefcc45a57d8f270f51 -->
