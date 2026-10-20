# 版本与仓库边界

[English](../EDITIONS.md) | 简体中文 · [文档目录](README.md)

HolePunch 面向自托管。Cloud 是独立开源产品，有独立提交、测试和发布流程，两个项目均采用 Apache-2.0。

| 仓库 | 内容 |
|---|---|
| HolePunch | 服务端/客户端核心、Self-hosted、CLI、可选桌面、共享 Web 资源和协议 |
| HolePunch-Cloud | 多用户应用组装、注册/邮件/邀请、用户和额度、通用运维文档及租户测试 |

HolePunch 可独立构建。Cloud 通过 Git 子模块固定完整核心提交；升级依赖必须显式测试，而不是自动跟随分支。两个仓库的部署凭据、CI 秘密和用户数据都应放在 Git 之外。

## 共享接口

核心提供身份、资源所有权、可选限速、存储、域名和转发。Cloud 通过扩展路由组装业务处理器；Self-hosted 不包含 SMTP 传输和 SaaS 处理器。

共享 Web 资源根据 `/api/v1/capabilities` 展示菜单，服务端独立检查权限。共享代码中的 Cloud 能力名称是接口约定，并非对 Cloud 实现的依赖。参阅 [Self-hosted API](API.md)。

## 数据与行为

Self-hosted 使用一个管理员，没有公开注册或数量配额，默认不限速并支持完整自定义域名。Cloud 支持多用户，隧道在用户额度内分配带宽。两者均不统计累计流量，也不设置月流量上限。

schema 4 记录数据库所属版本。两个服务端均拒绝打开对方的数据目录，没有自动转换；升级恢复见[部署指南](DEPLOYMENT.md)。

## 桌面客户端

桌面是共享客户端核心的可选公开应用，维护在本仓库。服务端与 CLI 构建不需要 GUI 依赖。当前桌面绑定表单使用长期设备令牌，一次性接入请用 CLI，详见[桌面开发](DESKTOP.md)。

英文原文规范和国际化要求适用于共享代码及 UI，Cloud 更新依赖时取得相应修改。

公开源码和稳定发布仍需完成[发布检查](RELEASE.md)。清理工作区不会清除 Git 历史。

<!-- translation-source: docs/EDITIONS.md; source-sha256: 7dd9fc484ad189b247e109ce34e1821562adff567705cfe331fb4aa05dff9947 -->
