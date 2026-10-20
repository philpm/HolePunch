# 开发与测试

[English](../DEVELOPMENT.md) | 简体中文 · [文档目录](README.md)

## 工具链与构建

使用 Rust 1.91.1+、Python 3 和 C/C++ 构建工具。Web 资源通过 `include_str!` 嵌入，修改后需重建服务端。服务端不需要 Node；UI 测试和可选桌面使用 Node.js 24。

```sh
cargo build --workspace --locked
python3 scripts/check-repository.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/sync-i18n.py --check
node --test tests/i18n.test.cjs
```

默认工作区应用为 `apps/selfhost` 和 `apps/cli`，分别生成 `holepunch-server`、`holepunch-client`。桌面有独立工作区和锁文件，见[桌面指南](DESKTOP.md)。此仓库不构建 Cloud。

代码注释、文档原文、诊断和示例使用英文，GUI 文本遵循[国际化流程](I18N.md)。生成的词条包提交到仓库，使普通 Rust 构建无需前端工具链。

## 数据平面工具

从官方发行获取适配平台的 Caddy 2.11.4，放到 `output/tools/caddy`（Windows 为 `.exe`）。rathole 必须包含本项目补丁及热重载支持：

```sh
./scripts/prepare-sidecar.sh
```

脚本验证上游 rathole 0.5.0 归档、应用 `scripts/patch-rathole.py` 和固定锁文件，并构建 server/client/noise/hot-reload 功能。输出为 `apps/desktop/src-tauri/binaries/holepunch-rathole-<target>`，本地开发需复制到 `output/tools/rathole`。macOS ARM64 示例：

```sh
mkdir -p output/tools
cp apps/desktop/src-tauri/binaries/holepunch-rathole-aarch64-apple-darwin output/tools/rathole
chmod +x output/tools/caddy output/tools/rathole
```

Sidecar 路径与桌面打包共享，但该脚本无需 Node/Tauri。`output/sidecar-source/` 保留上游构建源码用于发布许可证清单。下游版本 `0.5.0-holepunch.2` 保持线协议帧格式，将 UDP 接收缓冲扩大到 65,507 字节。

## 本地实例

```sh
python3 scripts/dev-local.py start
python3 scripts/dev-local.py stop
```

控制台为 `http://localhost:13012`，随机初始凭据位于 `output/local-selfhost/ACCESS.md`。首次使用无需环境配置，停止会保留数据和绑定，之后启动复用。`--edition selfhost` 只是兼容选项，不能选择 Cloud。

端口：控制平面 13012、Caddy Admin 12040、Noise 12354、HTTPS 18464、HTTP 18101、公网池 25100–25120。网关另分配内部回环端口。凭据、数据库、证书、进程信息和日志都留在忽略的实例目录中。

通过 [CLI](CLI.md) 接入。开发时将 `HP_RATHOLE_BIN` 设为绝对路径；默认客户端目录为 `~/.holepunch`，用 `--data-dir` 隔离测试。

## 真实链路回归

```sh
python3 scripts/e2e-selfhost.py --caddy output/tools/caddy --rathole output/tools/rathole
python3 scripts/e2e.py --caddy output/tools/caddy --rathole output/tools/rathole
```

第一套覆盖单管理员、不限速/可选限速、自定义/随机域名和 CLI 接入；第二套覆盖 TCP/UDP/HTTPS、热重载、撤销、备份恢复及本机网站，见[测试指南](TESTING.md)。

测试使用独立临时目录、回环端口和内部 CA，不修改系统信任。先停止占用内部端口的开发实例；Cloud 可能冲突，需在其仓库停止，并在测试后按需恢复。测试退出会清理自己的子进程。

报告写入 `output/selfhost/latest.json` 和 `output/e2e/latest.json`，指向临时目录中的完整日志。报告可能包含本地路径，原始目录可能包含秘密，分享前必须脱敏。

## 界面检查

检查中英文桌面/移动布局、键盘导航、打开的对话框、表单草稿和用户输入的 Unicode。兼容路由仍嵌入 `web/legacy`，移动资源时同步修改 Rust 路径。桌面要求见[桌面指南](DESKTOP.md)。

产物生成、依赖审查和签名要求见[发布指南](RELEASE.md)，已有工作流或旧扫描文件不代表当前验收通过。

## 部署工具测试

```sh
python3 -m unittest discover -s tests -p 'test_*.py' -v
python3 tests/smoke-native.py --bin-dir target/debug --caddy output/tools/caddy --rathole output/tools/rathole
```

单元测试使用临时目录，不安装主机服务。POSIX 冒烟测试在隔离目录安装真实二进制，检查初次与重启后的就绪和管理员登录，并验证 SIGTERM 后监听清理。使用动态端口，不修改正常开发实例。跨平台 sidecar 源码构建使用 `python3 scripts/build-rathole.py`，Bash 入口也委托给该脚本。

<!-- translation-source: docs/DEVELOPMENT.md; source-sha256: 0a3f6e4d5d249eed58a06244c434275577953144fa7fded2bd4d59f3458a018a -->
