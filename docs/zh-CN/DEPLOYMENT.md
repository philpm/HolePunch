# 部署与运维

[English](../DEPLOYMENT.md) | 简体中文 · [文档目录](README.md)

先阅读[三种安装方式](INSTALLATION.md)：交互式原生安装、克隆构建运行和 Docker Compose。本页介绍运行时配置与运维。安装器管理的单元是 `holepunch.service`，下文三个独立服务单元属于高级手动模板；同一部署只选一种进程管理方式。

本仓库构建 Self-hosted 和共享 CLI，数据库 schema 为 4。Cloud 位于独立 Apache-2.0 仓库，详见[边界说明](EDITIONS.md)。首次启动通过 `HP_BOOTSTRAP_EMAIL` 和 `HP_BOOTSTRAP_PASSWORD` 初始化一个管理员，没有注册或 SMTP 服务。升级前备份数据，不能直接打开 Cloud 数据目录。

下面 Compose 示例使用独立 Linux 监听端口，Docker 是可选方案。原生部署或已有 Nginx/宝塔的主机还应阅读[共存指南](COEXISTENCE.md)，不要公开受管隧道的内部回环端口。

## 前置条件

- 配备 Docker Engine、Compose v2 和足够源码构建磁盘空间的 Linux VPS。
- 公网 IP，以及控制台和 Web 隧道的 DNS A/AAAA 记录；IPv6 不通时不添加 AAAA。
- TCP 80、443、2333 和所需隧道端口可达；UDP 隧道需要对应 UDP 规则。
- 默认配置下，80、443、2019、2333 和 3000 不与已有监听冲突。

Compose 使用 `network_mode: host`，使 Caddy 可访问仅监听回环地址的 rathole Web 端口。目标环境为 Linux；Docker Desktop 主机网络有独立设置和限制，不是本部署的验收环境。

## 首次启动

```sh
python3 scripts/setup.py --public-host edge.example.com --panel-domain panel.example.com
python3 scripts/compose.py up
docker compose ps
docker compose logs -f center rathole
```

替换示例域名。Setup 拒绝覆盖已有 `.env`，创建权限为 0600。`HP_PUBLIC_ADDR` 是 rathole 连接地址，不是控制台 URL。初次建库后，公网地址、Caddy API 和 Ask URL 保存在数据库中，应通过控制台修改；修改其初始环境变量不会覆盖数据库。

Center 生成 `server.toml`；Caddy 先用仅监听回环地址的 Admin 配置启动，再接收受管路由。rathole 等待 Center 存活并读取共享配置目录。初始配置尚未生成时，由 Compose 重启策略重试。

Caddy 为显式配置的控制台域名签发证书，首次访问可能需要等待。启动包装器在就绪后输出访问地址、初始凭据和私密摘要路径。新安装登录后进入设置向导；用 `.env` 中生成的 `HP_BOOTSTRAP_EMAIL` 和 `HP_BOOTSTRAP_PASSWORD` 登录。初始化变量不会重置已有密码，不要将凭据粘贴到 issue 或日志。

没有控制台域名时：

```sh
python3 scripts/setup.py --public-host 203.0.113.10
python3 scripts/compose.py up
# Run on your own computer; replace the server address.
ssh -L 3000:127.0.0.1:3000 user@203.0.113.10
```

在本机打开 `http://127.0.0.1:3000`。客户端也可使用 SSH 转发后的 URL，但长期使用应配置公网 HTTPS 控制台。

## 第一条隧道

1. 使用初始管理员凭据登录，在**客户端**中创建设备。
2. 执行显示的 CLI 命令、输入一次性接入码，并保持运行。
3. 在**隧道**中选择设备、协议、本地地址和端口；带宽留空表示不限速，0 表示禁用。
4. Web 隧道可输入完整自定义域名并解析到入口服务器，也可在**域名设置**配置后缀后随机分配。
5. 检查配置状态、心跳，并实际访问应用。桌面客户端可选，其一次性接入流程尚未实现。

控制台默认英文，可用语言选择器切换简体中文并在当前浏览器保存。CLI 和服务端诊断仍使用英文。

## 环境变量

| 变量 | 默认值或用途 |
|---|---|
| `HP_BOOTSTRAP_EMAIL` / `HP_BOOTSTRAP_PASSWORD` | 必需的初始管理员账户，密码至少 12 字节 |
| `HP_ADMIN_TOKEN` | 必需，至少 32 字节，推荐 64 个随机十六进制字符 |
| `HP_DATA_DIR` | 本地 `data`；镜像中为 `/data` |
| `HP_BIND` | `127.0.0.1:3000`，Center HTTP 监听 |
| `HP_PUBLIC_ADDR` | 初始 `127.0.0.1:2333`，之后通过控制台修改 |
| `HP_RATHOLE_BIND` | `0.0.0.0:2333` |
| `HP_CADDY_URL` | 初始 `http://127.0.0.1:2019` |
| `HP_TLS_ASK_URL` | 初始 `http://127.0.0.1:3000/api/v1/tls/check` |
| `HP_PANEL_DOMAIN` | 可选控制台完整域名，禁止隧道占用 |
| `HP_PORT_START` / `HP_PORT_END` | `20000` / `20100`，含端点，需在 1024–65535 内 |
| `HP_CADDY_LISTEN` | `127.0.0.1:2019`，受管配置的 Admin 监听 |
| `HP_HTTPS_LISTEN` / `HP_HTTP_PORT` | `:443` / `80`，本地测试可覆盖 |
| `HP_TLS_INTERNAL` | 只有 `1` 启用本地测试 CA，不自动安装信任；公网生产不可启用 |
| `RUST_LOG` | `info` |

修改 Center 端口还需修改 Ask URL。控制台代理跟随 `HP_BIND`，将通配监听转换为回环地址。修改 Caddy Admin 端口时，启动配置、`HP_CADDY_LISTEN` 和数据库中的 Caddy URL 必须一致。

## 完整备份与恢复

若通过文件系统一致地备份数据库、WAL、Noise 密钥和证书，应先停止服务：

```sh
docker compose stop
# Actual volume names include the Compose project prefix.
docker volume ls
```

备份实际的 `holepunch-data`、`caddy-data`、`caddy-config` 卷和受保护的 `.env`，可用临时容器挂载并归档。备份含服务密钥，必须加密，不要只复制运行中的 `.db` 而遗漏 WAL。

使用相同镜像版本恢复到空卷，应用数据保留 UID/GID 10001 属主，然后启动 Compose。Center 启动时重置客户端心跳并重新应用数据库配置。

`docker compose down -v` 会删除持久配置和证书，仅在明确需要清空所有数据时使用。

## 升级与回滚

1. 停止服务并备份。
2. 更新源码和锁文件，执行 `docker compose build`。
3. 执行 `docker compose up -d`，检查日志并验证真实隧道流量。
4. 回滚时同时恢复匹配版本的数据备份与二进制。schema 4 支持迁移部分历史单管理员数据，不直接转换 Cloud 目录，也不自动降级 schema。

## 在线数据库备份

```sh
HP_DATA_DIR=data target/release/holepunch-server backup /secure/backups/holepunch.db
# Stop Center first; restore refuses while it is running.
HP_DATA_DIR=data target/release/holepunch-server restore /secure/backups/holepunch.db
```

备份使用 SQLite Online Backup，包含已提交的 WAL 数据；目标文件必须不存在。恢复会校验完整性和 schema、取得独占目录锁，并将此前数据库保留为 `before-restore-*.db`。备份需另行加密，环境文件和 Caddy 证书卷需单独备份。

`/healthz` 表示进程存活；`/readyz` 还要求 15 秒内成功同步、版本一致、rathole 控制监听可达。Compose 启动依赖使用存活检查，避免 rathole 与就绪检查相互等待；外部监控应使用就绪检查。

## 原生 systemd 部署

原生归档带有 systemd 模板。创建无登录 shell 的 `holepunch` 用户，将二进制安装到 `/usr/local/bin`，以 0600 权限创建 `/etc/holepunch/center.env` 或 `client.env`，安装相应单元。公网入口还需独立的 Caddy 2.11.4 服务，以 `deploy/caddy.json`（归档根目录为 `caddy.json`）启动。同一套服务不要同时使用 Compose 和原生单元。

使用 `deploy/systemd/holepunch-caddy.service` 时，将验证过的 Caddy 安装到 `/usr/local/bin/caddy`，创建无登录权限的 `hpcaddy` 用户和其 0700 目录 `/var/lib/holepunch-caddy`，把启动配置复制到 `/etc/holepunch-caddy/bootstrap.json`，目录权限 0755、文件 0644。

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now holepunch-caddy holepunch-center rathole
curl -f http://127.0.0.1:3000/readyz
```

不要再启动发行版自带的 `caddy.service`。专用单元用 `CAP_NET_BIND_SERVICE` 以非 root 身份绑定 80/443，证书持久化在其独立目录；应把该目录和 `/etc/holepunch/center.env` 纳入备份。

`deploy/compose.release.yml` 使用相邻的 `bootstrap.Caddyfile`，要求真实发布镜像、实际的 `HP_IMAGE_PREFIX` 和固定 `HP_VERSION`，没有虚构默认镜像仓库。Compose 使用只读根文件系统和有大小上限的容器日志。

## 排障

| 现象 | 检查项 |
|---|---|
| 配置同步失败 | Caddy Admin 可达性、配置目录写权限、2019 端口冲突；重新应用并检查日志 |
| 配置已应用但 TCP 不通 | rathole 热重载、端口占用、防火墙和客户端运行状态 |
| 客户端在线但应用不通 | 心跳仅说明 API 可达；检查 `rathole.log` 和本地服务 |
| UDP 不通 | 云厂商与操作系统都需 UDP 规则，TCP 规则不够 |
| HTTPS 502 | 对应 rathole Web 监听和本地 HTTP 应用；Web 当前代理明文 HTTP 上游 |
| 证书失败 | DNS/IPv6、80/443、时钟、CA 频率限制、域名是否属于有效且已启用的设备隧道 |
| 手工 Caddy 修改消失 | 该实例受程序管理，自动同步会修复配置偏差 |
| 创建端口失败 | 端口池耗尽/冲突；外部监听可能仅在 rathole 日志中报错 |
| 桌面找不到 rathole | 执行 prepare-sidecar 或设置绝对路径 `HP_RATHOLE_BIN` |
| 凭据存储失败 | 检查 Keychain/Credential Manager/Secret Service，或关闭记住绑定 |

同步失败不会撤销已保存编辑。修复节点后，自动重试会应用最新期望配置。

云厂商防火墙与主机 UFW 是不同层。如果已有监听且 UFW 放行，但外部 SYN 没到达，应检查云安全组或上游网络。管理面板可以使用 CDN 代理；rathole 公网地址应使用 VPS IP 或绕过普通 HTTP CDN 的独立域名。

与其他应用共用 80/443 时，通过**本机网站**登记并让应用 HTTP 监听回环地址。阅读[共存指南](COEXISTENCE.md)，不要手工向受管 Caddy 追加网站配置。

<!-- translation-source: docs/DEPLOYMENT.md; source-sha256: dcd9a4bb8a29c568a8bdb7a54d0627ebb3d456d5d48d3166d2bc3d80242e7c1f -->
