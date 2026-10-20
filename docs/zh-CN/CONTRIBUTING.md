# 参与贡献

[English](../../CONTRIBUTING.md) | 简体中文 · [文档目录](README.md)

请提交范围清晰的修改和可复现问题，先阅读[文档](README.md)、[架构](ARCHITECTURE.md)及[安全政策](SECURITY.md)。问题报告包含版本、平台、部署方式、复现步骤和脱敏日志。

## 语言与国际化

源码注释、标识符、文档原文、提交/PR 描述、CLI 帮助和诊断使用英文，英文文档是规范依据。译文位于 `README-zh.md` 和 `docs/zh-CN/`；其他非英文源码文本仅放在词条目录或有意设计的 Unicode 测试样例中。社区可使用自己熟悉的语言提问，语言熟练程度不是贡献门槛。

每条新增 Web/GUI 消息都应可翻译，同时更新英文与简体中文，保留占位符并测试两种语言。不要翻译用户名称、域名、API 字段、命令或原始日志，见[国际化指南](I18N.md)。

## 验证

```sh
python3 scripts/check-repository.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/sync-i18n.py --check
node --test tests/i18n.test.cjs
```

数据平面或协调逻辑修改还需 `scripts/e2e-selfhost.py` 与 `scripts/e2e.py`，覆盖限速、撤销、重连和恢复。桌面修改需独立 crate 检查与原生构建；Web 修改需检查桌面/移动布局、键盘、主题和语言，包括未保存表单及打开对话框。

数据库变更应有迁移测试并保留已发布数据兼容性，旧二进制必须拒绝新 schema。明确未测试平台，已有工作流不能证明 CI 通过。

不要提交环境文件、数据库、生成 TOML、密钥或原始日志。生成证据放在忽略目录，现行指南描述现行行为，过时材料留在适当历史中。移动文件时同步更新链接、Rust 嵌入路径、打包和国际化资源。

讨论保持尊重并聚焦技术，不发布个人信息或可利用的安全细节。维护者应在公开仓库前启用私密漏洞报告。

## 社区与许可

遵循[行为准则](CODE_OF_CONDUCT.md)和[支持政策](SUPPORT.md)，从功能分支向 `main` 提交，按[维护指南](MAINTAINING.md)将内部归档历史隔离在公开分支之外。

除非明确另有说明，有意提交并纳入项目的贡献依照 [LICENSE](../../LICENSE) 第 5 节采用 Apache-2.0。保留第三方许可证和署名，本仓库不要求额外贡献者协议。

<!-- translation-source: CONTRIBUTING.md; source-sha256: 2bc686efd488d5137cb931db76bafe924e66a85cd945972ce386fb3da63659f9 -->
