# HolePunch

<img src="docs/assets/logo.svg" alt="HolePunch 标志" width="96">

[![许可证：Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Rust 1.91.1+](https://img.shields.io/badge/Rust-1.91.1%2B-orange.svg)

[English](README.md) | [简体中文](README-zh.md)

使用 Rust 编写、通过 Web 控制台管理的自托管内网穿透工具。

HolePunch 借助一台具有公网 IP 的服务器，将 NAS、开发机等 NAT 后设备上的服务开放到公网。设备连接由 [rathole](https://github.com/rathole-org/rathole) 承载，HTTPS 由 [Caddy](https://github.com/caddyserver/caddy) 提供。通过 CLI 接入设备后，即可在浏览器中管理隧道。

> **开发版本：0.4.0-dev.1。** 目前可从源码安装。版本下载和预构建镜像需要相应版本实际发布后才能使用；尚未发布签名桌面安装包。

- [特性](#特性)
- [快速开始](#快速开始)
- [配置](#配置)
- [文档](#文档)
- [从源码构建](#从源码构建)
- [开发计划](#开发计划)
- [参与贡献](#参与贡献)
- [开源协议](#开源协议)

## 特性

- **TCP、UDP 和 Web 隧道。** 通过公网端口或 HTTPS 域名访问本地服务，支持 WebSocket 应用。
- **浏览器管理。** 在同一个控制台中创建设备与隧道、管理域名、查看状态并进行诊断。
- **一次性接入码。** 使用短期有效的接入码绑定设备，CLI 会保存该设备的凭据并自动同步隧道配置。
- **自己的服务器和域名。** 支持自定义域名、指定后缀下的随机子域名，也可接入同一台 VPS 上已有的网站。
- **可选带宽限制。** 隧道默认不限速，设置限速后由双向流量及所有连接共享；没有产品层面的数量配额或累计流量上限。
- **中英文界面。** Web 和可选桌面客户端可切换英文、简体中文，并保留表单草稿。
- **原生或容器部署。** 支持原生安装、源码构建和 Docker Compose；Self-hosted 版本使用 SQLite、内嵌控制台和单管理员工作区，可独立运行。

需要注册、邮件邀请、多用户和用户配额时，请使用 [HolePunch Cloud](https://github.com/philpm/HolePunch-Cloud)。两个项目均采用 Apache-2.0，并共用同一个 CLI。

## 快速开始

准备一台具有公网 IP 的服务器，以及运行本地服务的设备。下面以将 NAS 上的 `127.0.0.1:8080` Web 应用发布到 `https://nas.example.com` 为例。

### 1. 启动服务端

在已安装 Git、Python 3.10+、Docker Engine 和 Compose v2 的 Linux 服务器上执行：

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
python3 scripts/setup.py --public-host edge.example.com \
  --panel-domain panel.example.com --admin-email admin@example.com
python3 scripts/compose.py up
```

将示例域名替换为自己的域名，并将 `edge.example.com`、`panel.example.com` 和 `nas.example.com` 解析到服务器。放行用于 HTTPS 的 TCP 80/443，以及用于设备连接的 TCP 2333；TCP/UDP 隧道还需放行所选的公网端口。`edge.example.com` 应使用仅 DNS 解析。

服务就绪后，命令会输出控制台地址和初始管理员凭据。打开 `https://panel.example.com`，登录并完成**设置向导**。妥善保存 `.env` 和 `.env.access.txt`；采集启动日志时使用 `--hide-secrets`。

不使用 Docker，或服务器已经运行其他网站时，请阅读[安装指南](docs/zh-CN/INSTALLATION.md)和[网站共存指南](docs/zh-CN/COEXISTENCE.md)。

### 2. 接入设备

在**客户端**页面创建设备。在 NAS 上准备与其平台匹配的原生包，将 `holepunch-client` 和 `rathole` 保持在同一个 `bin/` 目录中，把该目录加入 `PATH`，然后执行：

```sh
holepunch-client connect --server https://panel.example.com
```

按提示输入一次性接入码，并保持客户端运行。没有可用的发布包时，可按[开发指南](docs/zh-CN/DEVELOPMENT.md)构建 CLI 和打过补丁的 sidecar。[CLI 手册](docs/zh-CN/CLI.md)介绍了后台服务与凭据存储。

### 3. 创建隧道

在**隧道**页面选择设备，创建 Web 隧道：

| 字段 | 值 |
|---|---|
| 名称 | `nas-web` |
| 协议 | Web |
| 本地地址 | `127.0.0.1` |
| 本地端口 | `8080` |
| 域名 | `nas.example.com` |
| 带宽 | 留空表示不限速 |

保存并等待配置同步后，访问 `https://nas.example.com`。本地应用需持续运行。TCP 或 UDP 隧道则使用控制台显示的公网主机和端口。

## 配置

设备、隧道、域名和可选限速均在控制台中管理。服务端自动生成 rathole 和 Caddy 配置，手动修改生成文件会被覆盖。

监听地址、数据目录、控制台域名等部署配置保存在 `.env` 或原生安装的受保护 `config.json` 中，详见[环境变量参考](docs/zh-CN/DEPLOYMENT.md#环境变量)。更改端口后，仍需配置匹配的 DNS、防火墙和入口路由。

### 日志与排障

```sh
docker compose logs -f center rathole caddy
holepunch-client status
holepunch-client doctor
holepunch-client logs --follow
```

`/healthz` 检查进程存活，`/readyz` 检查本地控制平面的就绪状态；还应实际访问隧道后的服务。[部署指南](docs/zh-CN/DEPLOYMENT.md)包含原生 systemd 日志、备份、升级和回滚说明。

## 文档

| 指南 | 内容 |
|---|---|
| [安装](docs/zh-CN/INSTALLATION.md) | 原生安装器、源码构建与 Docker Compose |
| [部署与运维](docs/zh-CN/DEPLOYMENT.md) | 配置、备份、升级和排障 |
| [CLI](docs/zh-CN/CLI.md) | 设备接入、后台服务和诊断 |
| [架构](docs/zh-CN/ARCHITECTURE.md) | 共享核心、转发机制与目录职责 |
| [API](docs/zh-CN/API.md) | 会话、设备、隧道、设置与健康接口 |
| [国际化](docs/zh-CN/I18N.md) | 界面词条与文档翻译 |

[完整中文文档](docs/zh-CN/README.md) · [English documentation](docs/README.md) · [安全说明](docs/zh-CN/SECURITY.md)

## 从源码构建

需要 Rust 1.91.1+、Python 3.10+ 和本地 C/C++ 构建工具。UI 测试与可选桌面客户端需要 Node.js 24；服务端构建会直接嵌入 Web 资源。

```sh
cargo build --workspace --locked
cargo test --workspace --locked
```

按[开发指南](docs/zh-CN/DEVELOPMENT.md)准备 Caddy 和打过补丁的 rathole，再执行 `python3 scripts/dev-local.py start`。本地控制台为 `http://localhost:13012`，生成的凭据保存在 `output/local-selfhost/ACCESS.md`。

## 开发计划

当前重点是各目标平台的发布验收、桌面一次性接入和诊断导出。远程节点及自动路由选择属于后续工作。详见[路线图](docs/zh-CN/ROADMAP.md)和[测试要求](docs/zh-CN/TESTING.md)；在取得本项目的实际测量结果之前，不作性能比较承诺。

## 参与贡献

欢迎提交问题报告、范围清晰的 PR 和翻译。请阅读[贡献指南](docs/zh-CN/CONTRIBUTING.md)、[支持说明](docs/zh-CN/SUPPORT.md)和[行为准则](docs/zh-CN/CODE_OF_CONDUCT.md)。安全漏洞请按[安全说明](docs/zh-CN/SECURITY.md)中的私密渠道报告。

## 开源协议

HolePunch 采用 [Apache-2.0](LICENSE)。第三方组件保留各自的许可证，详见[第三方声明](docs/zh-CN/NOTICE.md)。感谢 rathole、Caddy 和 Tauri 社区提供的基础组件。

<!-- translation-source: README.md; source-sha256: 3c6f4ffa23b0fe3c69cd1c1c4818a3a1f228ea6cb187e35119ecf1daf46a5e1c -->
