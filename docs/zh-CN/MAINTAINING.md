# 维护公开仓库

[English](../MAINTAINING.md) | 简体中文 · [文档目录](README.md)

`main` 是公开集成分支。使用范围清晰的功能分支和 PR，合并前要求 CI 通过并完成审阅。发布应绑定不可变标签和干净、经过测试的工作区。参阅[贡献](CONTRIBUTING.md)、[测试](TESTING.md)及[发布](RELEASE.md)。

## 公开与本地材料

源码、通用部署工具、示例、文档和测试进入 `main`。客户信息、生产端点、访问凭据、私密事故报告、原始测试证据及内部验收日记不进入。运行数据保存在 Git 之外受保护、被忽略的目录；忽略规则无法保护已经被跟踪的文件。

本地归档分支可以保留历史计划和运营笔记。不要推送归档分支，不要把其历史合并到 `main`，不要使用 `git push --all` / `git push --mirror`。公开 GitHub 仓库中的分支即使名称含“private”也仍然公开。凭据应在秘密存储中，而不在任何分支；泄露后需轮换，删除文件不会撤销凭据。

首次公开前审阅全部远端分支、标签、Release 文件、Actions 产物和可达历史。干净的本地 `main` 不会移除远端已有材料。仅发布经过审阅的引用；若需替换远端历史，应先协调并获批，再对明确引用使用 `--force-with-lease`，提前保存受保护本地备份。

## GitHub 设置

在可用时启用私密漏洞报告、秘密扫描/推送保护、Dependabot 告警，以及要求 CI 和 PR 审阅的分支规则。工作流默认只读权限，不向 fork PR 暴露部署秘密。仓库里的工作流文件不会代替账户设置。

宣布公开前，验证[安全说明](SECURITY.md)中的私密报告链接，在新克隆上检查构建/安装说明，并确认许可证识别为 Apache-2.0。

## 依赖与发布变更

审阅锁文件修改和上游许可证，执行相应测试，在[更新记录](CHANGELOG.md)记录用户可见变化。应用锁文件应提交以保证可复现。生成词条包也提交，使服务端不需要前端工具链；始终从源 JSON 生成。

CI 对每次 PR 修订和 `main` 推送各运行一次，新修订会取消过时任务；需要单独重跑分支时使用 `workflow_dispatch`。Linux 服务端任务固定使用 Ubuntu 24.04，避免运行器镜像迁移静默改变构建环境。

Dependabot 合并兼容的 Cargo/npm 更新及 Actions 更新，破坏性 Cargo/npm 升级单独审阅。Rust 编译器保留文档中的 1.91.1 基线；调整前需确认编译器已发布，并审阅最低支持版本。`dtolnay/rust-toolchain` 标签可能早于编译器发布，因此排除其自动版本标签升级。

不要把 Actions 产物标为已发布稳定版本。上传发布文件前验证实际归档、校验和、版本、平台、许可证清单和安装流程。

参考：[GitHub 仓库维护实践](https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories)、[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)。

<!-- translation-source: docs/MAINTAINING.md; source-sha256: cd02a59e17a0fe0263f4c1ca87d511523f89404241c8bf4f90a790d1b25894eb -->
