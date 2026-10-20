# Web 资源

[English](../../web/README.md) | 简体中文 · [文档目录](README.md)

- `portal.html`、`portal.js`、`portal.css`：当前会话认证控制台，根据能力展示功能，权限由服务端校验。
- `style.css`：当前与兼容页面共享基础样式。
- `legacy/index.html`、`legacy/app.js`：兼容令牌控制台，仍通过 `/legacy` 和 `/app.js` 提供，供历史未初始化模式使用；Self-hosted 默认使用 portal。
- `i18n.js`、`locales/*.json`、`locales/catalogs.js`：共享语言运行时、源词条和生成离线包，桌面也使用它们。

服务端在 `crates/server-core/src/lib.rs` 嵌入资源，没有前端打包器，构建服务端无需 Node。修改词条后执行 `python3 scripts/sync-i18n.py` 并重建服务端，见[国际化](I18N.md)。

兼容资源仍有真实路由与回归用途，移除需独立决定 API 兼容政策。桌面专用 UI 在 `apps/desktop/ui`。

<!-- translation-source: web/README.md; source-sha256: eef3e2cbd8331195a5b4438916cdcdaf114fe056f6426a5f1bad64850bd64909 -->
