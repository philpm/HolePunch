# 项目架构

[English](../ARCHITECTURE.md) | 简体中文 · [文档目录](README.md)

仓库依赖与版本边界见[版本与仓库边界](EDITIONS.md)。

| 目录 | 职责 |
|---|---|
| `crates/server-core` | Axum 身份/资源 API、SQLite schema 4、可选限速网关、Caddy/rathole 协调、本地节点、备份恢复 |
| `apps/selfhost` | 按 SelfHosted 策略组装服务端，不依赖 Cloud 应用 |
| `crates/client-core` | 设备绑定、配置同步、租约、日志及子进程生命周期 |
| `apps/cli` | 共享命令行客户端与后台服务安装 |
| `apps/desktop` | 复用客户端库的可选 Tauri 应用，独立 Cargo 工作区 |
| `web` | 按能力展示的共享控制台、国际化运行时与词条 |

Cloud 在独立 Apache-2.0 仓库实现多用户处理器，通过 `router_with_routes` 扩展固定提交版本的核心 API。Self-hosted 不导入或组装 Cloud 路由。UI 能力检查控制展示，服务端独立强制执行权限校验。

## 数据与限速

设备与隧道共享资源 ID 和所有权模型。Self-hosted 使用一个管理员，没有数量或总带宽额度。schema 4 在 `tunnel_unlimited` 中记录不限速隧道，保留原有限速存储兼容性，0 仍表示禁用。

不限速隧道跳过速率等待，但保留连接/队列保护、授权撤销和租约。有限速隧道的双向流量及全部连接共享一个预算，增加连接数不能放大预算。不持久化累计流量，也不计费。

## 界面语言

英文原文作为 `web/locales/en.json` 的键，由各语言目录提供翻译。内嵌控制台和桌面共用离线运行时与生成的词条包；桌面打包时复制资源，托盘标签也来自同一目录。API 标识和序列化字段不随语言变化，详见[国际化](I18N.md)。

## 节点演进

目前仅实现 LocalNode。全球节点发现、网络质量测量、路由选择和故障切换属于未来工作。应先扩展共享节点协议，再由 Cloud 控制平面编排运营策略。

<!-- translation-source: docs/ARCHITECTURE.md; source-sha256: b04cb6ae086ffdcfabc9349f1f86c1508ae691cf11c88ae2d7382766294bf842 -->
