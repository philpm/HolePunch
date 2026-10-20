# CLI 手册

[English](../CLI.md) | 简体中文 · [文档目录](README.md)

`apps/cli` 中的 CLI 使用 `crates/client-core`，连接 Self-hosted 和 Cloud 的命令相同，只需更换 `--server` URL。CLI 本身就是客户端，无需桌面应用。每台承载隧道服务的设备都需持续运行客户端进程。

## 安装与连接

解压管理员提供的、适配设备平台的原生包，将 `bin/holepunch-client` 和 `bin/rathole` 放在一起，把目录加入 PATH，或使用可执行文件的绝对路径。普通用户不需要 Rust、Docker 或手写 TOML。目前尚无公开生产下载，应使用管理员实际提供的平台版本。

在控制台创建设备，然后执行：

```sh
holepunch-client connect --server https://panel.example.com
```

按提示输入一次性接入码，10 分钟后过期且只能兑换一次。长期设备令牌保存在本地，不放在命令或下载 URL 中。脚本可通过 `--token-stdin` 输入接入码；不建议在共享终端使用兼容选项 `--enroll-token`。

绑定后，`holepunch-client connect` 复用已保存凭据。隧道的创建、编辑、启停和删除在 Web 控制台进行。Ctrl-C 停止前台进程但保留绑定。CLI 帮助、提示和诊断为英文，界面语言选择不会改变终端输出。

## 命令

| 命令 | 行为 |
|---|---|
| `connect [--server URL]` | 兑换接入码或运行已绑定设备 |
| `status` | 显示进程、绑定、最近同步和隧道状态，不显示令牌 |
| `logs [--follow]` | 读取数据平面日志，核心日志保留约 4 MiB |
| `doctor` | 只检查 API、设备授权、控制端口和本地 TCP 服务；UDP 需实际请求 |
| `disconnect` | 请求当前 CLI 停止并等待确认，保留绑定 |
| `logout` | 停止并移除本地绑定/服务配置；服务端撤销需在 Web 操作 |
| `version` / `--version` | 显示版本 |
| `service install/start/stop/uninstall` | 显式管理 macOS LaunchAgent 或 Linux 用户 systemd 服务 |

所有命令支持 `--data-dir PATH`，默认 `~/.holepunch`，也可通过 `HP_CLIENT_DATA` 配置。Unix 目录权限为 0700、凭据文件为 0600。CLI 使用受保护的本地文件，与桌面的系统钥匙串独立。不要使用共享或同步目录；进程锁防止同一目录重复运行客户端。

`HP_RATHOLE_BIN` 覆盖 sidecar 路径，否则读取 CLI 可执行文件旁的 `rathole`。旧的 `HP_API_URL` / `HP_CLIENT_TOKEN` 接入方式保留兼容，但不是默认流程。

## 后台运行

先绑定一次，用 Ctrl-C 停止前台进程，然后运行：

```sh
holepunch-client service install
holepunch-client service start
holepunch-client status
```

macOS 使用当前用户的 LaunchAgent；Linux 使用 `systemctl --user`，退出登录后继续运行需要管理员启用 lingering。Windows 当前使用前台 `connect`，尚无经过验证的服务安装器。服务安装记录可执行文件、数据目录和 sidecar 的绝对路径，移动文件后需重新安装服务。

`service stop` 停止后台任务，`service uninstall` 移除自动启动。`disconnect` 停止当前进程但不卸载服务。升级时先停止服务，在同一目录替换客户端和 sidecar，再重启。解除绑定与移除自动启动是两个独立操作。

## 恢复与排障

- 永久撤销设备会删除本地绑定，需要在控制台重新接入。Cloud 账户冻结/解冻遵循平台策略。
- 临时断网会自动重试；配置授权 90 秒未刷新后暂停转发，服务端另有 15 秒策略租约。
- Self-hosted 可按隧道限速，没有账户总额度；Cloud 遵循平台用户额度。
- `doctor` 提示本地服务不可达时，检查应用是否监听配置的地址/端口。API 正常但控制端口不通时，检查代理和防火墙；客户端不修改系统级网络设置。
- 成功命令退出码为 0，命令、配置或网络错误为非零。对外仅分享版本、脱敏错误和复现步骤，不要分享接入码、`binding.json` 或 `client.toml`。

<!-- translation-source: docs/CLI.md; source-sha256: fff4db2675117e23a9b7652eb8e833dfd2e0eadff237d2f3fc063501b1d61653 -->
