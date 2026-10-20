# 安装 HolePunch

[English](../INSTALLATION.md) | 简体中文 · [文档目录](README.md)

可选择三种安装方式，运行的都是同一套 Self-hosted 服务端、内嵌 Web 控制台、打过补丁的 rathole 和独立 Caddy 实例。原生服务端不依赖 Docker、Node.js、外部数据库或桌面客户端。

| 方式 | 适用场景 | 依赖 |
|---|---|---|
| 交互式原生安装器 | 安装经过校验的二进制包、配置监听地址，并可启动 Linux 服务 | Python 3.10+、本地归档或已发布版本、HTTPS 下载连接 |
| 克隆、构建并运行 | 开发、审计或在二进制发布前部署 | Git、Rust 1.91.1+、Python 3.10+、平台 C/C++ 构建工具 |
| Docker Compose | 已经使用容器的 Linux 服务器 | Docker Engine、Compose v2；源码构建示例还需要 Git |

**发布状态：** 开发仓库不代表已有公开发布包、下载地址或镜像。源码构建可在发布前使用，安装器也接受本地归档及校验文件。`--version` 只有在指定版本实际发布后才有效，不会回退到其他版本或任意构建源码。

## 支持的环境

| 主机 | 原生安装和源码构建 | 后台运行 |
|---|---|---|
| Linux x86-64 / ARM64，glibc | 安装器支持这些目标；需匹配发布包，或自行构建 | 在 systemd 主机上由安装器管理服务 |
| macOS Apple Silicon / Intel | 可移植原生安装和前台运行器 | 保持运行器启动，暂不自动安装 launchd 服务 |
| Windows x86-64 | Python/PowerShell 安装和原生二进制；源码构建需要 MSVC | 保持运行器启动，暂不自动安装 Windows 服务 |
| Alpine/musl、其他架构 | 暂无预构建安装支持约定 | 需自行构建并验证 |

生产服务端以 Linux 为目标。systemd 安装使用 `useradd` 和 `systemctl`，不依赖 apt/dnf，可用于安装了这些命令的 Debian/Ubuntu、RHEL 系主机。安装器不会安装 Python、修改软件源、关闭 SELinux 或修改防火墙；发行版策略可能仍需管理员配置。Linux x86-64 发布构建基于 Ubuntu 22.04（glibc 2.35+），ARM64 基于 Ubuntu 24.04（glibc 2.39+）。较旧的兼容发行版应本地构建，不能假设较新的二进制可直接运行。

macOS 原生生命周期已做本地检查，可移植回归套件也配置了 Windows 和 Linux CI。配置了 CI 作业并不代表对应平台已经完成验收，详见[测试指南](TESTING.md)。

## 安装前准备

1. 将 `panel.example.com` 等控制台域名解析到服务器。rathole 设备连接使用单独的 `edge.example.com` 或公网 IP。
2. 确认由谁占用公网 TCP 80/443。HolePunch 的 Caddy 可通过**本机网站**代理已有应用；已有 Nginx/宝塔面板的服务器应先阅读[共存指南](COEXISTENCE.md)。
3. 在主机和云厂商防火墙中放行控制端口（默认 TCP 2333）及实际使用的 TCP/UDP 隧道端口。不要公开控制台原始 HTTP 端口、Caddy Admin 和内部 Web 网关端口。
4. 将应用数据和证书保存在本地持久磁盘。

原生安装器会检查 TCP/UDP 监听冲突，出现冲突时停止安装，保留已有应用运行。它也拒绝覆盖已有安装目录、配置文件或服务单元，不会自动修改防火墙、DNS、已有 Caddy、Nginx、PHP 或宝塔配置。

## 1. 交互式原生安装器

### 从仓库或本地构建包安装（目前可用）

按[发布打包指南](RELEASE.md)构建归档，将归档及相邻 `.sha256` 文件传到服务器。复制 `scripts/deploy.py`、`scripts/access.py` 和对应平台的包装脚本，或直接使用归档内的文件。

在 Linux systemd 服务器上，以 root 权限安装到系统目录：

```sh
sudo python3 scripts/deploy.py install \
  --archive /path/to/holepunch-selfhost-VERSION-TARGET.tar.gz \
  --prefix /opt/holepunch --service systemd
```

脚本会询问公网主机、可选控制台域名、管理员邮箱和监听/隧道端口，展示目标目录后确认安装，并生成随机密码和令牌。Caddy 2.11.4 从上游发布下载，使用其公布的 SHA-512 校验；已有验证过的二进制可通过 `--caddy /path/to/caddy` 指定。

本地归档必须匹配主机架构和 Self-hosted 版本。必须提供校验文件，可用 `--checksum PATH` 指定其他路径。校验和能检查损坏和文件不匹配，但不是独立的发布者签名；仅安装来自可信来源的包和脚本。

macOS 或 Linux 前台安装：

```sh
sh scripts/install.sh --archive /path/to/release.tar.gz
python3 "$HOME/.local/share/holepunch/deploy.py" run --prefix "$HOME/.local/share/holepunch"
```

Windows 需先将 Python 加入 PATH：

```powershell
python scripts/deploy.py install --archive C:\Downloads\release.zip --prefix C:\Users\YOUR_USER\HolePunch
python C:\Users\YOUR_USER\HolePunch\deploy.py run --prefix C:\Users\YOUR_USER\HolePunch
```

`scripts/install.ps1` 也是入口，透传相同选项。Windows 安装器在写入凭据前限制安装目录 ACL；Unix 使用 0700 目录权限和 0600 凭据文件权限。systemd 模式下，二进制仍归 root 所有，专用服务用户仅有配置读取权限和 `state/` 写入权限。

### 发布版本后的单命令安装

从固定发布标签下载脚本，保存后再执行，保留交互输入。将 `vX.Y.Z` 替换为真实发布标签：

```sh
export HOLEPUNCH_VERSION=vX.Y.Z
curl -fsSLo install.sh "https://raw.githubusercontent.com/philpm/HolePunch/$HOLEPUNCH_VERSION/scripts/install.sh" && sudo env HOLEPUNCH_VERSION="$HOLEPUNCH_VERSION" sh install.sh --prefix /opt/holepunch --service systemd
```

包装脚本从同一标签获取 Python 安装器，再下载匹配平台的归档和校验文件。需预先安装 Python 3.10+ 和 curl，没有浮动的 `latest` 默认值。交互脚本应先下载，不要直接通过管道传给 `sh`。

发布标签存在后，可在 PowerShell 中运行：

```powershell
$env:HOLEPUNCH_VERSION = 'vX.Y.Z'
Invoke-WebRequest "https://raw.githubusercontent.com/philpm/HolePunch/$env:HOLEPUNCH_VERSION/scripts/install.ps1" -OutFile install.ps1
.\install.ps1 --prefix "$HOME\HolePunch"
```

遵守所在组织的 PowerShell 执行策略，也可以直接运行 Python 安装器。

### 自动化和自定义端口

所有交互项都有命令行参数。无人值守安装必须明确指定 `--non-interactive`、主机、邮箱，以及归档/版本/源码来源：

```sh
sudo python3 scripts/deploy.py install --non-interactive \
  --archive /path/to/release.tar.gz --caddy /path/to/caddy \
  --prefix /opt/holepunch --service systemd \
  --public-host edge.example.com --panel-domain panel.example.com \
  --admin-email admin@example.com \
  --center-port 13000 --caddy-port 12019 --control-port 12333 \
  --http-port 18080 --https-port 18443 --port-start 21000 --port-end 21100
```

自定义端口会同步用于控制台、TLS 授权 URL、Caddy Admin 启动配置和 rathole 连接地址。**只修改 80/443 并不会自动提供域名共享路由或公网证书验证。** 若公网 URL 使用默认端口，已有入口必须将对应流量转发到这些监听地址；也可将受管 Caddy 作为公网入口，把已有网站迁移到回环地址。选择拓扑前阅读[共存指南](COEXISTENCE.md)。

### 启动、停止和检查

```sh
sudo systemctl status holepunch
sudo journalctl -u holepunch -f
sudo systemctl restart holepunch
sudo systemctl stop holepunch
sudo python3 /opt/holepunch/deploy.py check --prefix /opt/holepunch
```

单个服务监督三个子进程。任一子进程退出时，运行器停止其他子进程并报错退出，随后由 systemd 重启整套服务。前台模式按 Ctrl+C 停止全部子进程，保留数据和凭据。运行器等待 `/readyz` 真正成功后才报告就绪；日志写入终端或 journal，请按主机策略设置日志保留周期。

完成信息会显示浏览器地址和初始凭据，也可在受保护的 `config.json` 中读取 `HP_BOOTSTRAP_EMAIL`、`HP_BOOTSTRAP_PASSWORD`。初始邮箱、密码和管理员 API 令牌还保存到同目录的受保护 `ACCESS.txt`。自动化或采集终端日志时使用 `--hide-secrets`，不要公开访问摘要。有控制台域名时访问其 HTTPS 地址；无域名时可通过 `ssh -L 3000:127.0.0.1:3000 user@edge.example.com` 转发，再在本机打开 `http://127.0.0.1:3000`。

## 2. 克隆、构建并运行

克隆仓库并选择准备部署的发布标签或提交。仓库尚未公开时，需要正常的 GitHub 授权凭据。

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
# Check out a real release tag or reviewed commit before a production build.
python3 scripts/deploy.py install --source . --prefix "$HOME/.local/share/holepunch"
python3 "$HOME/.local/share/holepunch/deploy.py" run --prefix "$HOME/.local/share/holepunch"
```

源码安装会以 `cargo build --release --locked` 构建服务端与 CLI，下载校验固定的 rathole 源码、应用仓库补丁、使用固定锁文件构建 sidecar，再获取经过校验的 Caddy 并进入相同配置流程。下载 Rust 依赖和上游源码需要联网。Web UI 由 Rust 内嵌，服务端无需 Node 构建步骤。

构建依赖：

- Linux：Rust 1.91.1+、Python 3.10+、C/C++ 编译器、链接器和发行版开发工具；Debian/Ubuntu 可使用 `build-essential`，RHEL 系使用 GCC 和开发工具。
- macOS：Rust、Python 3.10+ 和 Xcode Command Line Tools。
- Windows：Rust MSVC 工具链、Python 3.10+、Visual Studio C++ Build Tools 及 Windows SDK；必要时用 `python` 替代 `python3`。

分别构建组件以用于打包或自定义进程管理器：

```sh
cargo build --release --locked -p holepunch-server -p holepunch-cli
python3 scripts/build-rathole.py
# Install Caddy 2.11.4 separately, or let the native installer obtain it.
```

可执行文件为 `target/release/holepunch-server`、`target/release/holepunch-client` 和 `apps/desktop/src-tauri/binaries/holepunch-rathole-TARGET`，Windows 带 `.exe`。`scripts/prepare-sidecar.sh` 保留为兼容 Bash 入口。自行管理进程时参考[环境变量与手动 systemd 模板](DEPLOYMENT.md)。

生产部署应以普通用户构建并[打包](RELEASE.md)，再以 root 执行归档安装器，避免以 root 编译未经审阅的源码。上述流程无需 Docker。

## 3. Docker Compose

### 从克隆的源码构建（目前可用）

此部署使用 Linux 主机网络，使 Caddy 能连接仅监听回环地址的 Web 网关端口。Docker Desktop 不是该方案的生产验收环境。

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
python3 scripts/setup.py --interactive
docker compose config --quiet
python3 scripts/compose.py up
docker compose ps
docker compose logs -f center rathole caddy
```

也可不通过交互生成配置：

```sh
python3 scripts/setup.py --public-host edge.example.com \
  --panel-domain panel.example.com --admin-email admin@example.com
```

`setup.py` 只写入一次包含私密凭据的 `.env`。它支持原生安装器相同的七类端口/范围选项，Caddy 启动配置和健康检查会跟随自定义端口。Setup 校验范围和重叠，但不能检查远程 Docker 主机的监听状态，应在 `up` 前自行检查。Compose 不会修改云厂商防火墙。

`scripts/compose.py up` 执行 Compose、等待 `/readyz`，然后输出管理 URL、初始邮箱/密码/API 令牌、配置路径和首次使用说明。`--hide-secrets` 隐藏终端中的秘密值，受保护的 `.env.access.txt` 保留原始访问信息。启动或就绪失败时不会打印成功信息。

也支持直接使用 `docker compose up -d --build`，之后以 `python3 scripts/access.py --env-file .env` 查看访问信息；该命令是只读凭据查询，不执行健康检查。

源码 Compose 构建服务端和打过补丁的 rathole，Caddy 使用固定上游镜像，不依赖已发布的 HolePunch 镜像。两种 Compose 配置都会传入初始管理员凭据，并通过命名卷持久化应用数据与证书。

### 使用发布镜像（发布后）

从同一版本获取 `compose.release.yml` 和 `bootstrap.Caddyfile`，放在同一目录。原生归档根目录也提供 `setup.py`、`compose.py`、`deploy.py` 和 `access.py`，可在该目录运行 `python3 setup.py --interactive`，设置镜像前缀和版本后执行 `python3 compose.py up --pull`。

也可通过 `scripts/setup.py --output /path/to/that/directory/.env` 生成受保护的 `.env`，添加真实镜像仓库前缀和固定标签，再从该目录执行：

```sh
# Use actual published image names; these variables have no fictional default.
export HP_IMAGE_PREFIX=YOUR_REGISTRY/YOUR_NAMESPACE/holepunch
export HP_VERSION=YOUR_PUBLISHED_TAG
docker compose -f compose.release.yml config --quiet
docker compose -f compose.release.yml pull
docker compose -f compose.release.yml up -d
# Then display local access details with access.py from the same release.
python3 access.py --env-file .env
```

在源码仓库中可用 `python3 scripts/compose.py up --file deploy/compose.release.yml --env-file .env --pull` 拉取镜像、检查就绪并显示摘要，也支持直接执行 `docker compose --env-file .env -f deploy/compose.release.yml ...`。绑定路径相对于 Compose 文件，因此 `bootstrap.Caddyfile` 必须放在旁边。当前发布工作流生成用于验证的 OCI 构建产物，不会自动推送镜像仓库。

## 首次浏览器登录

三种安装方式进入同一 Web 设置向导。用完成信息中的凭据登录，新建 Self-hosted 数据库会自动打开**设置向导**。登录页和控制台顶部均有**语言**选择器；默认英文，中英文选择按浏览器保存，切换时保留表单值。

向导分四步：

1. 确认管理员账户与显示名称，可替换生成的密码；修改需提供当前密码，保存后注销所有会话。
2. 确认公网控制地址及可选域名后缀，按说明配置 DNS、公网端口及已有网站。控制台域名来自 `HP_PANEL_DOMAIN`，修改需重启服务。
3. 检查本地配置同步和 rathole 控制监听。检查使用已保存配置，不是未保存草稿，也不能证明公网 DNS、防火墙或端到端隧道可用。
4. 查看客户端命令、接入码流程和首条隧道说明，然后保存并进入**客户端**。

最终一步以事务原子保存配置。**稍后完成**保留未完成状态并显示提醒，未保存草稿不会跨刷新保留。完成标记存储在服务端数据库，换浏览器不会重复设置。已有数据库保留配置，可通过侧栏手动打开向导。设置状态 API 不返回密码或 API 令牌。

部署摘要记录的是**初始**凭据；控制台中修改密码不会回写 `.env`、`config.json` 或访问摘要。管理员 API 令牌不能作为客户端令牌，必须在控制台创建设备取得它的一次性接入码。

## 后续使用与运维

在控制台创建设备，在运行本地应用的机器上执行 `holepunch-client connect --server https://panel.example.com` 并输入接入码。保持 CLI 运行，在控制台配置隧道、域名和可选带宽限制，详见 [CLI](CLI.md)。

备份、恢复和监控见[运维指南](DEPLOYMENT.md)。原生安装的可变状态位于 `PREFIX/state/`，应与受保护的 `PREFIX/config.json` 和部署版本一同备份。文件系统备份前先停止服务，不要只复制单个文件来替换正在运行的 SQLite 数据库。

安装器只处理**全新安装**，不负责原地升级。升级应先停机、备份 `state/` 和 `config.json`、验证新归档，再仅替换 `bin/` 下匹配的二进制和必要的运行器，启动后检查就绪与实际流量。systemd 安装应保留属主和权限。不要重跑首次设置、重新生成凭据或假设数据库可降级。回滚需要匹配的数据备份；启动失败时安装目录与日志会保留以供诊断。

<!-- translation-source: docs/INSTALLATION.md; source-sha256: b355fb228e6d702a53b316c288f6387403431be8b217f108f8d0bcd2400a7902 -->
