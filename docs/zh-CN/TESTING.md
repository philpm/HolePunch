# 测试

[English](../TESTING.md) | 简体中文 · [文档目录](README.md)

按[开发指南](DEVELOPMENT.md)准备工具并执行命令。检查应针对正在审阅的提交，旧验收报告不能证明新版本通过。

| 范围 | 必需检查 |
|---|---|
| Rust 工作区 | `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`、`cargo build --workspace --locked` |
| 仓库和文档 | `python3 scripts/check-repository.py` |
| 部署工具 | `python3 -m unittest discover -s tests -p 'test_*.py' -v` |
| 国际化 | `python3 scripts/sync-i18n.py --check`、`node --test tests/i18n.test.cjs` |
| 原生安装器 | `python3 tests/smoke-native.py --bin-dir target/debug --caddy output/tools/caddy --rathole output/tools/rathole` |
| Self-hosted 数据平面 | `python3 scripts/e2e-selfhost.py --caddy output/tools/caddy --rathole output/tools/rathole` |
| 共享恢复与转发 | `python3 scripts/e2e.py --caddy output/tools/caddy --rathole output/tools/rathole` |

集成套件使用隔离临时目录和真实子进程。先准备校验固定的 Caddy 和打过补丁的 rathole，在不影响无关服务的前提下释放冲突端口。覆盖 TCP/UDP/HTTPS、限速、撤销、重连、本机网站和备份恢复；不代表公网 DNS、SMTP 或全部平台支持已验证。

## 界面验收

检查英文默认、手动选择简体中文、刷新后保持选择、键盘操作、无障碍标签、移动端溢出和亮/暗主题。在未保存表单和打开对话框时切换语言，保留用户名称、域名、HTML 特殊字符、占位符花括号及 Unicode。检查已知 API 错误和原始诊断回退，见[国际化](I18N.md)。

桌面有独立 Cargo 工作区，按[桌面指南](DESKTOP.md)构建检查，并在目标平台验证原生托盘、凭据存储和进程生命周期。模拟浏览器桥接不能证明原生行为正确。

## 证据和发布验收

原始报告、截图、凭据和临时日志放到被忽略的 `output/` 或仓库之外。公开 PR 只应包含脱敏结果、测试提交、平台、命令和限制，不提交生产验收日记或客户部署细节。

CI 执行可移植部署、Rust、国际化、Compose 构建和 Linux 集成检查。发布还需要真实平台安装、升级/回滚、公网 TLS、持续负载以及依赖/许可证审查。工作流文件不等于成功运行，见[发布指南](RELEASE.md)。

<!-- translation-source: docs/TESTING.md; source-sha256: 1928bb0cdc42bfb54229dd709ca4869bc104ced3e2cff7167078aa8d9b5cec1e -->
