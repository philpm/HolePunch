# 与其他网站共用 VPS

[English](../COEXISTENCE.md) | 简体中文 · [文档目录](README.md)

在控制台的**本机网站**中发布同一服务器上的应用。Caddy 为控制台、Web 隧道和本机网站管理 HTTPS。本机网站不经过客户端和 rathole，因此撤销客户端后仍可运行。

## 新服务器的推荐布局

| 公网入口 | 后端 |
|---|---|
| panel.example.com | 127.0.0.1:3000 上的 Center |
| blog.example.com | 127.0.0.1:8080 上的本地博客 |
| app.example.com | 127.0.0.1:8081 上的本地应用 |
| nas.example.com | Web 隧道 → rathole → 内网 NAS |

Caddy 占用公网 80/443，其他 Web 服务器监听回环端口，无需公网防火墙规则。若后端按 Host 路由，多个网站可以共用应用端口，但域名必须唯一。

## 添加网站

1. 启动本地 HTTP 应用，例如 `127.0.0.1:8080`，在 VPS 上用 `curl -I http://127.0.0.1:8080` 检查。
2. 将 `blog.example.com` 的 A/AAAA 记录解析到 VPS；没有可用 IPv6 时不要加 AAAA。
3. 打开**本机网站 → 添加网站**，填写名称、域名、回环地址和 HTTP 端口。
4. 保存，等待**配置已应用**，然后访问 `https://blog.example.com`；首次签发证书需要正确 DNS、公网 80/443 以及可用 CA。
5. 可编辑、禁用、重启用或删除。禁用/删除会移除路由并拒绝新证书授权，但不撤销已有证书，也不改变应用进程和文件。

网站记录参与数据库版本、审计、在线备份和恢复。重启 Caddy、编辑隧道或撤销客户端不会删除网站记录。

## 容器应用

将容器 HTTP 端口映射到主机回环地址，在控制台填写主机端口：

```yaml
services:
  app:
    image: your-verified-app:fixed-version
    ports:
      - "127.0.0.1:8080:3000"
```

替换为真实镜像和内部端口。默认 Linux Compose 使用主机网络，使 Caddy 能连接该地址。控制台中不要填写容器 IP 或 Docker 服务名。

## PHP、Nginx 与已有面板

HolePunch 代理 HTTP，不执行 PHP，也不支持直接连接 FastCGI，因此不要填写 PHP-FPM 的 9000 端口。先用 Nginx/Apache 在回环 HTTP 端口提供完整 PHP 站点，再将其加入**本机网站**。

例如将 Nginx 站点监听改为 `listen 127.0.0.1:8080;`，保留 `server_name`、网站根目录和 FastCGI 配置，由 Caddy 管理证书。正确配置应用公网 HTTPS URL 和可信代理，避免 Cookie 错误、绝对链接错误或重定向循环；仅信任本地入口提供的转发头。

已有宝塔面板时，先备份配置、逐个验证 HTTP 后端，再安排公网入口迁移。面板可能重新生成 Nginx 配置；HolePunch 不会修改面板或自动导入网站。当已有 Nginx 占用 80/443 时，默认 Caddy 无法绑定相同端口。

## 限制与排障

- 仅支持精确域名到回环 HTTP 后端：`127.0.0.1` / `::1`；API 中 localhost 归一化为 `127.0.0.1`。应用自行负责认证、文件、数据库及进程运行。
- HTTPS 和 WebSocket 使用 Caddy 标准反向代理。共存集成检查覆盖 TLS 上的 HTTP，不代表已完成独立 WebSocket 压测。
- UI 不提供通配域名路由、路径路由、远程/HTTPS 后端或任意 Caddy JSON。
- 网站、隧道和控制台域名互斥，禁用仍预留域名；迁移域名前需编辑或删除原记录。
- 后端端口不得占用 Center、Caddy Admin、HTTP/HTTPS 入口、rathole 控制及隧道公网监听。隧道分配也会避开已登记网站端口，未登记外部应用的监听需自行检查。
- **启用**是配置状态，**配置已应用**仅说明入口接受配置，不代表应用健康。HTTPS 502 时检查应用进程、监听和日志。
- Caddy 是独立受管实例，手动配置会被同步覆盖，请通过控制台登记网站。
- 全部应用共享 CPU、内存、磁盘和带宽，按真实总负载规划容量。

当前 schema 为 4，升级前备份。回滚需要匹配版本的数据，旧二进制不能打开新 schema，详见[部署指南](DEPLOYMENT.md)。

## 安装工具

[原生安装器](INSTALLATION.md)检查所选服务和隧道端口，冲突时拒绝安装而不停止已有应用。原生和 Compose 设置均支持自定义控制台、Caddy Admin、控制、HTTP/HTTPS 和隧道范围，并生成一致的内部 URL 与启动配置。Compose 设置能验证配置，但不能检查远程 Docker 主机监听；两者均不修改已有 Web 服务器或防火墙。

另选 HTTP/HTTPS 端口并不会让已有入口自动支持 HolePunch 域名路由或证书挑战。使用标准公网 URL 前，应完成上文所述入口拓扑。

<!-- translation-source: docs/COEXISTENCE.md; source-sha256: a1344b6aa54bd3f83a25c0024ed70186eda018c3534ef459a0536e295b439869 -->
