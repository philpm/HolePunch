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

[标志源文件](../assets/logo.png)以圆形屏障与两端网络节点表达连接，以斜向线段表达穿透。橄榄绿图形和白色底色由两个项目、浏览器控制台与桌面应用共同使用。源文件为 1024 × 1024 的不透明正方形 PNG，按仓库的 Apache-2.0 协议分发。

透明版本采用几何 SVG 重绘，使用同心圆、圆弧及圆头线段；透明 PNG 从该矢量图导出，图形保持一致。所有品牌资源均按同一 Apache-2.0 协议分发。

| 资源 | 用途 |
| --- | --- |
| [橄榄绿 SVG](../assets/logo.svg) | 浅色背景上的可缩放图形，可编辑的矢量源文件 |
| [透明橄榄绿 PNG](../assets/logo-transparent.png) | 1024 × 1024，带真实 Alpha 透明通道 |
| [白色 SVG](../assets/logo-light.svg) | 深色或彩色背景上的可缩放图形 |
| [透明白色 PNG](../assets/logo-light.png) | 1024 × 1024，带真实 Alpha 透明通道的白色图形 |

SVG 使用原生几何图形，`viewBox` 为 `0 0 1024 1024`，没有背景、嵌入位图、字体或外部依赖。使用时保持正方形比例和四周留白。自定义颜色只需修改根元素的 `color` 属性，内部图形使用 `currentColor`。内联 SVG 也可用 CSS `color` 改色；通过 `<img>` 加载时，使用 SVG 自身的根元素颜色，不会继承页面文字颜色。橄榄绿对比度不足时，选用白色版本。

替换白底源图或编辑 `docs/assets/logo.svg` 后，使用已安装的 Tauri CLI 重新生成需要提交的 PNG、ICO、ICNS 和透明版本：

```sh
npm ci --prefix apps/desktop
python3 scripts/sync-icons.py
```

脚本从 `logo.png` 生成平台图标，从 `logo.svg` 生成透明 PNG 和白色 SVG。桌面构建会自动将 `web/logo.png` 复制到界面目录。重新构建服务端与桌面应用即可嵌入更新后的图标。更新 Cloud 的核心子模块时，也应将全部五份标志资源复制到 Cloud 仓库的 `docs/assets/`。

<!-- translation-source: apps/desktop/README.md; source-sha256: 341b682a6369a01af2d0b56280481a2f120eafefcd2d8744d2bdaccb68ad3d76 -->
