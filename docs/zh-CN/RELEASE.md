# Self-hosted 发布指南

[English](../RELEASE.md) | 简体中文 · [文档目录](README.md)

核心/CLI 版本为 **0.4.0-dev.1**，桌面独立维护版本 **0.2.0-rc.2**。这些是开发构建，尚未发布签名安装器和公开镜像。验证要求见[测试](TESTING.md)，待完成工作见[路线图](ROADMAP.md)。

## 原生归档

按[开发指南](DEVELOPMENT.md)准备工具和 sidecar 源码，再构建：

```sh
cargo build --release --locked -p holepunch-server -p holepunch-cli
# The target must match the binaries; this example is macOS ARM64.
python3 scripts/package.py --target aarch64-apple-darwin --bin-dir target/release --rathole output/tools/rathole --out dist
```

产物为 `holepunch-selfhost-<version>-<target>.tar.gz`（Windows 为 ZIP）和 `.sha256`。归档包括服务端、CLI、rathole、可移植安装器/运行器、共享 `access.py`、独立 `setup.py`/`compose.py`、Shell/PowerShell 入口、发布版 Compose、Caddy 启动配置、中英文文档、systemd 模板、许可证清单和构建元数据。Caddy 由安装器下载校验或通过 `--caddy` 提供，桌面单独发布。

归档中的服务端安装名兼容为 `bin/holepunch-center`，实际来自 `holepunch-server`。`BUILD.json` 记录版本类型、schema、核心提交和版本号，不能只凭可执行文件名判断版本。应从已提交的干净仓库发布，保证记录与内容一致。

```sh
shasum -a 256 -c holepunch-selfhost-0.4.0-dev.1-aarch64-apple-darwin.tar.gz.sha256
tar -xzf holepunch-selfhost-0.4.0-dev.1-aarch64-apple-darwin.tar.gz
```

安装、升级、备份与回滚见[部署指南](DEPLOYMENT.md)。

## 工作流与桌面

手动 `.github/workflows/release.yml` 构建 Linux x64/ARM64、Windows x64、macOS ARM64/x64 归档及 Linux amd64 OCI 镜像文件。它上传 Actions 产物，不会自动创建 GitHub Release 或推送镜像。桌面安装包还需要 `include_desktop`、平台验收和签名。

`deploy/compose.release.yml` 必须使用真实发布的镜像前缀和固定版本，不应填写不存在的镜像。Cloud 有独立开源发布流程，Self-hosted 归档不含 SaaS 实现。

国际化运行时与词条嵌入服务端，桌面在 Rust 构建时复制同样资源，无需运行时翻译服务或下载。打包前执行 `python3 scripts/sync-i18n.py --check` 拒绝过期词条包。

## 提供公开下载

通过验收并获得发布授权后，创建与 `v<Cargo version>` 完全匹配的 GitHub Release 标签，上传保留生成文件名的归档及相邻 `.sha256`。安装器按平台选择并依赖这些命名。应从干净仓库发布，带未提交修改生成的包只能作为测试产物。

引导脚本从同一标签下载 `scripts/deploy.py` 和 `scripts/access.py`，需保证它们在对应源码中可访问。宣传单命令安装前，应通过真实公开 URL 在干净主机安装；产物未发布时保留源码和本地归档步骤。

镜像发布应将测试过的 center/rathole 镜像推送到同一仓库前缀和版本标签，同时分发 `compose.release.yml` 与 `bootstrap.Caddyfile`。需验证镜像部署下的新管理员登录，不能仅测源码方案。构建工作流不会自动公开仓库或发布镜像/Release。

## 依赖审查

```sh
python3 scripts/audit-dependencies.py
# Keep informational advisories as REVIEW; ordinary vulnerabilities still block.
python3 scripts/audit-dependencies.py --allow-informational
```

工具扫描当前核心、桌面和 rathole 锁文件，结果写入忽略的 `output/dependency-audit-current.json`。旧快照不是当前证据，清单也不能证明漏洞在特定二进制中可利用或可达。每次发布前重新扫描，解决阻塞问题并审阅提示。

## 发布要求

- 工作区测试、格式、Clippy 和两套真实链路回归通过。
- 国际化一致性/运行时检查通过，实际检查中英文 Web/桌面流程，翻译不能替换用户数据。
- 在真实系统验证目标平台原生部署、Linux Docker、升级回滚、公网 DNS/CA 和持续负载。
- 归档校验和、版本、依赖清单和许可证与实际二进制一致。
- 单独验证桌面凭据、自启动、退出、签名/公证，服务端验收不能覆盖桌面。
- 公开前审查源码与 Git 历史，建立私密漏洞报告渠道，产物排除凭据、数据库及内部运维材料。

工作流文件不能证明远端 CI 已运行。对应检查完成后，才能提升发布状态。

<!-- translation-source: docs/RELEASE.md; source-sha256: 50c2d86aaa1115e88d63ab4655690cd230fe0159227b0ef7c4792680176bd425 -->
