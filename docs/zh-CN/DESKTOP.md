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

<!-- translation-source: apps/desktop/README.md; source-sha256: 6d90e5dd84c2bed30eea4a812eaa942f387875d25534268f1a078fceb8df9da6 -->
