# 第三方声明

[English](../../NOTICE.md) | 简体中文 · [文档目录](README.md)

- [rathole 0.5.0](https://github.com/rathole-org/rathole/tree/v0.5.0)：Apache-2.0，版权归 Yujia Qiao 及贡献者。`scripts/patch-rathole.py` 调整构建元数据、终端检测、依赖和功能，把 UDP 接收缓冲从 2048 扩大到 65507 字节，不改变线协议帧格式。下游版本为 `0.5.0-holepunch.2`。
- [Caddy](https://github.com/caddyserver/caddy)：Apache-2.0，独立进程运行，不链接进 HolePunch 服务端二进制。
- [Tauri](https://github.com/tauri-apps/tauri)：MIT / Apache-2.0，用于可选桌面运行时和插件。

核心工作区、桌面和 rathole 各有 Cargo 锁文件。原生服务端/CLI 归档通过 Cargo metadata 生成核心/客户端/rathole 的依赖与许可证清单，收集可用的 LICENSE/COPYING/NOTICE/COPYRIGHT 文件和构建锁文件，不含桌面依赖清单；桌面分发需独立审查。

清单可能包含跨平台记录，不意味着每项依赖都链接进特定二进制。分发时保留适用许可证与声明，尚未发布签名安装器和公开镜像。

<!-- translation-source: NOTICE.md; source-sha256: 3a32c24f8c5fc08731fef61df1e43b4e0b7e721b116b3f6b0fb52a9f79c3d368 -->
