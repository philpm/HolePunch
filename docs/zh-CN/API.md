# Self-hosted API v1

[English](../API.md) | 简体中文 · [文档目录](README.md)

本页描述当前 Self-hosted 服务端。除健康探针外，下列路径均带 `/api/v1` 前缀。不开放 CORS，请求体限制为 16 KiB。应用错误通常返回 `{"error":"English description"}`：401 未认证、403 禁止、400 输入无效、404 记录不存在、409 冲突、429 限流、502 入口失败。框架解析错误可能是纯文本。描述是供人阅读的诊断，不是稳定标识；客户端应根据 HTTP 状态和操作上下文处理。GUI 翻译已知消息，未知诊断回退为英文。

## 管理员会话

首次启动初始化一个管理员。Self-hosted 没有注册、邮箱验证/重置或委托用户管理。登录返回 `hp_session` Cookie 和 CSRF 值，后续写操作发送 `X-CSRF-Token`。Cookie 属性为 HttpOnly/Secure/SameSite=Strict，24 小时过期。`HP_ALLOW_HTTP=1` 仅用于本地 HTTP 开发。

| 方法 | 路径 | 请求或响应 |
|---|---|---|
| GET | `/capabilities` | 公开；edition、multi_user、quotas、email_auth 等能力 |
| POST | `/auth/login` | `{email,password}` → `{user,csrf}` 和 Cookie |
| POST | `/auth/logout` | 当前会话与 CSRF，退出登录 |
| GET | `/me` | user、profile、csrf、capabilities、资源数量、域名后缀、公网连接地址 |
| PUT | `/me/profile` | `{display_name}`，最多 80 字节 |
| PUT | `/me/password` | `{current_password,password}`，成功后重新登录 |
| GET | `/me/sessions` | 自己的会话 `{id,current,expires}` |
| DELETE | `/me/sessions/{id}` | 撤销指定会话 |

密码为 12–256 字节，邮箱归一化为小写。邮箱是本地登录名，无需邮件服务。会话 ID 是摘要，不是登录令牌；修改密码不撤销设备。

Self-hosted 的 `limits.clients/tunnels/bps`、`allocated_bps`、`remaining_bps` 返回 `null`，表示没有产品配额；拒绝 `X-Manage-User` 委托请求。

## 客户端与隧道

| 方法 | 路径 | 请求或行为 |
|---|---|---|
| GET | `/portal/clients` | 工作区设备列表，不含令牌 |
| POST | `/portal/clients` | `{name}` → `{id,enroll_token,expires_in}` |
| POST | `/portal/clients/{id}/enrollment` | 重新生成接入码；兑换时轮换设备和服务令牌 |
| POST | `/portal/clients/{id}/revoke` | 撤销设备并删除其隧道 |
| POST | `/enroll` | 无需 Web 会话，以 `{token}` 兑换 `{client_id,token}` |
| GET | `/portal/tunnels` | 隧道列表，含 bandwidth_limit_bps 和 policy_status |
| POST | `/portal/tunnels` | 创建隧道 |
| PUT | `/portal/tunnels/{id}` | 替换可编辑字段 |
| DELETE | `/portal/tunnels/{id}` | 删除并释放端口/域名；Self-hosted 无域名冷却期 |
| POST | `/portal/bandwidth` | 原子设置 `{allocations:[{tunnel_id,bps},...]}` |

接入码 10 分钟过期且仅兑换一次。长期设备令牌用于客户端 API，不能登录管理员控制台。

```json
{
  "name": "food-api",
  "client_id": "device-id",
  "protocol": "web",
  "local_host": "127.0.0.1",
  "local_port": 8000,
  "domain": "food-api.example.com",
  "enabled": true,
  "bandwidth_limit_bps": null
}
```

- 协议为 `tcp`、`udp`、`web`。本地主机接受 IPv4、IPv6 或 localhost，端口范围 1–65535。
- Web 接受完整自定义域名；域名留空时需配置后缀以随机分配。域名使用 ASCII/Punycode，不接受 URL、路径、通配符或 IP 字面量。
- TCP/UDP 端点由 `/me.public_addr` 中的主机与响应中的 `public_port` 组成，Noise 控制端口不是服务端口。
- `public_port` 可自动分配或从节点池显式选择，各协议共享端口池且不能重复分配。
- `bandwidth_limit_bps` 为 `null` 或省略表示不限速，0 表示禁用，正整数单位为 bit/s；`allocations[].bps` 语义相同。
- 每条限速聚合双向和所有连接，没有账户分配总额度。调高限速或改为不限速不会自动重新启用已禁用隧道。

保存成功不等于服务可达。`policy_status` 表示网关策略应用状态；同步状态 pending/applied/error 描述配置同步，均不能替代实际应用探测。

## 域名与本机网站

`GET /domains` 返回 `{domain_suffix}`，`PUT /domains` 接受相同结构，空字符串清除后缀。完整自定义域名不受该后缀限制，DNS 仍需管理员配置。

本机网站不需要客户端：`GET/POST /sites` 查询/创建，`PUT/DELETE /sites/{id}` 更新/删除。

```json
{"name":"Blog","domain":"blog.example.com","upstream_host":"127.0.0.1","upstream_port":8080,"enabled":true}
```

仅允许回环 HTTP 后端，localhost 归一化为 127.0.0.1。禁用保留域名/端口预留，删除只移除配置而不删除应用。网站域名不能与控制台/隧道冲突，后端端口不能使用受保护的监听端口，详见[共存指南](COEXISTENCE.md)。

## 节点与运维

这些接口要求管理员会话，写操作还需 CSRF：

| 方法 | 路径 | 内容 |
|---|---|---|
| GET | `/dashboard` | 主机 CPU、内存、接口速率、心跳数量、同步状态 |
| GET / PUT | `/settings` | 节点配置，更新 caddy_url/public_addr/tls_ask_url |
| POST | `/settings/rotate-ask-key` | 轮换 Ask 密钥并同步，不返回密钥 |
| POST | `/sync` | 立即同步；失败返回 502，保留期望状态 |
| GET | `/diagnostics` | 版本、schema、配置和监听就绪状态 |
| GET | `/audit` | 最近 100 条事件，最多保留 1000 条 |

网卡速率包含其他本机应用，不是按隧道计费数据。后台每 3 秒同步一次。Applied 表示配置已写入且 Caddy 已接受，不代表所有隧道都健康。

## 客户端 API、TLS 和健康检查

设备发送 `Authorization: Bearer <device-token>`：

- `GET /client/config`：client_id、启用的隧道、完整 rathole TOML 和 revision；响应含服务密钥，必须保护。
- `POST /client/heartbeat`：`{"running":true}`，在线窗口 45 秒，running 只代表进程状态。

撤销后两者均返回 401。设备不能管理其他资源或节点设置。

- `GET /api/v1/tls/check?domain=nas.example.com&key=<ask-key>`：控制台、有效且启用的隧道或启用的本机网站返回 200，否则 403。
- `GET /healthz`：无需认证的存活检查。
- `GET /readyz`：无需认证的就绪检查，要求 15 秒内成功同步、版本一致及 rathole 控制监听可达，否则 503。

## 兼容性

核心保留旧 `/clients`、`/tunnels` 和 Web 资源用于历史集成回归。已初始化的 Self-hosted 对旧资源写入返回 409，新集成使用 `/portal/*`。

旧 Bearer 管理员令牌不是当前登录流程。显式设置 `HP_LEGACY_ADMIN=1` 可为受支持的管理端点启用兼容认证，但不会恢复旧资源写入，也不会挂载 SaaS 注册、邮件和用户管理路由。

## 管理员首次设置

`GET /api/v1/setup` 要求管理员 Web 会话，返回完成状态、管理员邮箱/显示名称、公网控制地址、域名后缀、部署控制台域名及监听/端口范围。不返回密码摘要、初始密码、管理员/Noise/证书令牌。两个版本的管理员 `/me` 响应都含 `setup_complete`，普通 Cloud 成员不接收此字段。

`PUT /api/v1/setup` 要求管理员会话和 CSRF，字段为 `display_name`、`public_addr`、`domain_suffix` 和可选 `current_password` / `password`。先验证全部字段，再通过单个事务保存资料、节点地址、后缀和完成状态。修改密码会验证当前密码、注销已有会话并返回 `login_required: true`；否则当前会话仍有效。配置同步仍为异步（`status: pending`），拒绝未知字段。

新数据库初始未完成。升级时，已有节点配置的数据库会被标记为完成并保留设置；向导可随时重新打开。普通 Cloud 成员不能使用这些管理员接口。

<!-- translation-source: docs/API.md; source-sha256: e5f7f5c793ada876b575af95e71e90c86e9a9ba216b924584699875bafcfed01 -->
