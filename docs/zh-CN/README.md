# 中文文档

[English](../README.md) | 简体中文 · [文档目录](README.md)

这些指南描述当前 Self-hosted 开发版本。英文是规范原文，中文文档与其对应维护；内容聚焦支持的行为和可重复验证流程，不收录内部验收日记。

| 任务 | 指南 |
|---|---|
| 了解功能与限制 | [产品概览](OVERVIEW.md) |
| 选择原生、源码或 Compose 安装 | [安装](INSTALLATION.md) |
| 部署、升级、备份或排障 | [部署与运维](DEPLOYMENT.md) |
| 与已有 Web/PHP 网站共用 VPS | [网站共存](COEXISTENCE.md) |
| 接入设备或后台运行 | [CLI 手册](CLI.md) |
| 本地构建和测试 | [开发](DEVELOPMENT.md) |
| 了解代码和数据平面 | [架构](ARCHITECTURE.md) |
| 调用当前 API | [API](API.md) |
| 验证功能与识别限制 | [测试](TESTING.md) |
| 构建产物与检查发布条件 | [发布](RELEASE.md) |
| 查看计划 | [路线图](ROADMAP.md) |
| 了解两个仓库及桌面的边界 | [版本与仓库边界](EDITIONS.md) |
| 添加或维护语言 | [国际化](I18N.md) |

[贡献](CONTRIBUTING.md) · [支持](SUPPORT.md) · [行为准则](CODE_OF_CONDUCT.md) · [安全](SECURITY.md) · [维护](MAINTAINING.md) · [第三方声明](NOTICE.md)

## 本地生成目录

`target/`、桌面 `target/` 和 `node_modules/` 是可重建缓存，`dist/` 存放生成的发布产物，均不提交。

`output/tools/` 存放本地 Caddy/rathole 二进制；`output/local-selfhost/` 包含开发实例的凭据、数据库和进程信息；`output/selfhost/`、`output/e2e/` 和 `output/playwright/` 包含测试证据。Sidecar 源码目录用于构建和许可证清单。这些都被 Git 忽略；运行数据和测试证据可能含秘密，不能直接作为公开 issue 附件。

清理运行中的实例前，先执行 `python3 scripts/dev-local.py stop` 并备份数据，不要把整个 `output/` 当作可随意删除的构建缓存。

还可参阅[桌面客户端](DESKTOP.md)、[Web 资源](WEB.md)和[更新记录](CHANGELOG.md)。

<!-- translation-source: docs/README.md; source-sha256: 7ba6f2a40852afce98344f0191feb13956a16f6bd3d1f0229e3999c2299ff2e9 -->
