# Install HolePunch

[English](INSTALLATION.md) | [简体中文](zh-CN/INSTALLATION.md)

Choose one of three paths. They run the same Self-hosted server, embedded web console, patched rathole and dedicated Caddy instance. Docker, Node.js, an external database and the desktop app are not required for a native server.

| Path | Best for | Requirements |
|---|---|---|
| Interactive native installer | Install verified release binaries, configure listeners, optionally start a Linux service | Python 3.10+, a release archive or published version; HTTPS access for downloads |
| Clone, build and run | Develop, audit or deploy before binary releases are published | Git, Rust 1.91.1+, Python 3.10+, platform C/C++ build tools |
| Docker Compose | A Linux host already using containers | Docker Engine, Compose v2, Git for the source-build example |

**Release availability:** this development checkout does not imply a published release, public download URL or registry image exists. Source builds work before publication. The installer can also consume a locally produced archive and its checksum. `--version` only works after that exact version has been published; it does not fall back to another version or build arbitrary source.

## Supported environments

| Host | Native installer and source builds | Background operation |
|---|---|---|
| Linux x86-64 / ARM64 with glibc | Supported installer targets; a matching release asset is required, or build from source | Installer-managed systemd service on a systemd host |
| macOS Apple Silicon / Intel | Portable native installation and foreground runner | Keep the runner active; launchd installation is not automated |
| Windows x86-64 | Python/PowerShell installer and portable native binaries; source builds require MSVC build tools | Keep the runner active; Windows Service installation is not automated |
| Alpine/musl, other architectures | No prebuilt installer contract yet | Build and validate separately |

The intended production server is Linux. The systemd path uses `useradd` and `systemctl`; it is independent of apt/dnf and applies to Debian/Ubuntu and RHEL-family hosts with those commands installed. It does not install Python, change repositories, disable SELinux or change the host firewall. Distribution policies may require administrator configuration. Linux x86-64 release builds use Ubuntu 22.04 (glibc 2.35+); ARM64 release builds use Ubuntu 24.04 (glibc 2.39+). Build locally on older compatible distributions instead of assuming a newer binary will run.

macOS native lifecycle checks run locally; the portable regression suite is also configured for Windows and Linux CI. A configured CI job is not evidence that an OS acceptance run has completed. See [validation](TESTING.md).

## Before any installation

1. Point a panel domain, such as `panel.example.com`, to the server. A separate `edge.example.com` or the public IP is used for the rathole control connection.
2. Decide who owns public TCP 80/443. HolePunch's Caddy can also serve existing applications through **Local websites**. For an existing Nginx/BT Panel server, read [coexistence](COEXISTENCE.md) first.
3. Allow the control port (default TCP 2333) and the TCP/UDP tunnel ports you actually use in both host and provider firewalls. Keep the panel's raw HTTP port, Caddy Admin and internal web gateway listeners private.
4. Use a local, persistent disk for application data and certificate storage.

The native installer checks TCP and UDP listeners before installing. A conflict stops installation and leaves the other application running. It also refuses to overwrite an existing installation directory, config file or service unit. No firewall, DNS, existing Caddy, Nginx, PHP or BT Panel configuration is changed automatically.

## 1. Interactive native installer

### From a checkout or locally built archive (available now)

Build a release archive using [release packaging](RELEASE.md). Transfer the archive and its adjacent `.sha256` file to the server. Copy `scripts/deploy.py`, `scripts/access.py` and the appropriate wrapper, or use their copies inside the archive.

On a Linux systemd server, run as root with a system install directory:

```sh
sudo python3 scripts/deploy.py install \
  --archive /path/to/holepunch-selfhost-VERSION-TARGET.tar.gz \
  --prefix /opt/holepunch --service systemd
```

The script asks for the public host, optional panel domain, administrator email and listener/tunnel ports, then shows the destination and asks before installation. It generates a random password and token. It downloads Caddy 2.11.4 from the upstream release and verifies its published SHA-512 checksum. To supply an already verified Caddy binary, add `--caddy /path/to/caddy`.

A local archive must match the host architecture and Self-hosted edition. Its checksum is mandatory; `--checksum PATH` selects a different checksum file. Download checksums detect corruption and mismatched files; they are not an independent signature of the publisher. Only install archives and scripts from sources you trust.

For macOS or a Linux foreground installation:

```sh
sh scripts/install.sh --archive /path/to/release.tar.gz
python3 "$HOME/.local/share/holepunch/deploy.py" run --prefix "$HOME/.local/share/holepunch"
```

For Windows, with Python on PATH:

```powershell
python scripts/deploy.py install --archive C:\Downloads\release.zip --prefix C:\Users\YOUR_USER\HolePunch
python C:\Users\YOUR_USER\HolePunch\deploy.py run --prefix C:\Users\YOUR_USER\HolePunch
```

`scripts/install.ps1` is an alternative entry point that forwards the same options. The installer restricts the Windows installation directory ACL before writing credentials. On Unix it creates the installation directory as 0700 and credentials as 0600. In systemd mode, root retains ownership of binaries and grants the dedicated service user read access to configuration and write access only to `state/`.

### One-command setup after a release is published

Download the installer from a pinned release tag and execute the saved script so terminal input remains available. Replace `vX.Y.Z` with a real published tag:

```sh
export HOLEPUNCH_VERSION=vX.Y.Z
curl -fsSLo install.sh "https://raw.githubusercontent.com/philpm/HolePunch/$HOLEPUNCH_VERSION/scripts/install.sh" && sudo env HOLEPUNCH_VERSION="$HOLEPUNCH_VERSION" sh install.sh --prefix /opt/holepunch --service systemd
```

The wrapper fetches the Python installer from the same tag, which downloads the matching platform archive and checksum. Python 3.10+ and curl must already be installed. There is no moving `latest` default. Do not pipe the interactive script into `sh`; download it first.

PowerShell bootstrap, after a published tag exists:

```powershell
$env:HOLEPUNCH_VERSION = 'vX.Y.Z'
Invoke-WebRequest "https://raw.githubusercontent.com/philpm/HolePunch/$env:HOLEPUNCH_VERSION/scripts/install.ps1" -OutFile install.ps1
.\install.ps1 --prefix "$HOME\HolePunch"
```

Follow your organization's PowerShell execution policy. Running the Python installer directly is also supported.

### Automation and custom ports

All prompts have command-line equivalents. An unattended installation must specify `--non-interactive`, the host, email and an artifact/source choice:

```sh
sudo python3 scripts/deploy.py install --non-interactive \
  --archive /path/to/release.tar.gz --caddy /path/to/caddy \
  --prefix /opt/holepunch --service systemd \
  --public-host edge.example.com --panel-domain panel.example.com \
  --admin-email admin@example.com \
  --center-port 13000 --caddy-port 12019 --control-port 12333 \
  --http-port 18080 --https-port 18443 --port-start 21000 --port-end 21100
```

Custom ports are applied consistently to the panel, TLS authorization URL, Caddy Admin bootstrap and rathole connection address. **Changing 80/443 alone does not provide shared domain routing or public certificate validation.** For default-port public URLs, the existing edge must forward the appropriate traffic to these listeners; otherwise use the managed Caddy as the public edge and migrate existing sites to loopback. Read [coexistence](COEXISTENCE.md) before choosing a topology.

### Start, stop and inspect

```sh
sudo systemctl status holepunch
sudo journalctl -u holepunch -f
sudo systemctl restart holepunch
sudo systemctl stop holepunch
sudo python3 /opt/holepunch/deploy.py check --prefix /opt/holepunch
```

The single service supervises all three processes. If one exits, the runner stops its other children and exits with an error; systemd restarts the stack. In foreground mode, press Ctrl+C to stop all three. Credentials and data are retained. The runner waits for real `/readyz` success before reporting ready. Logs stream to the terminal or journal; configure journal retention according to your host policy.

The completion message tells you where to open the browser and lists the initial credentials. You can also read `HP_BOOTSTRAP_EMAIL` and `HP_BOOTSTRAP_PASSWORD` in protected `config.json`. The installer displays the initial email, password and administrator API token, and saves a protected `ACCESS.txt` alongside configuration. Use `--hide-secrets` to hide the password/token in terminal output, particularly in automation. Never include this summary in public logs. Open the configured HTTPS panel domain. Without a domain, forward the local panel over SSH, for example `ssh -L 3000:127.0.0.1:3000 user@edge.example.com`, and open `http://127.0.0.1:3000` on your computer.

## 2. Clone, build and run

Clone the repository and select the release tag or revision you intend to deploy. Until the repository is public, GitHub requires your normal authorized Git credentials.

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
# Check out a real release tag or reviewed commit before a production build.
python3 scripts/deploy.py install --source . --prefix "$HOME/.local/share/holepunch"
python3 "$HOME/.local/share/holepunch/deploy.py" run --prefix "$HOME/.local/share/holepunch"
```

The source path builds the server and CLI with `cargo build --release --locked`, downloads checksum-pinned rathole source, applies the repository patches, builds the sidecar with its lockfile, obtains verified Caddy, and runs the same configuration flow. It needs Internet access to fetch Rust dependencies and upstream source. The web UI is embedded by Rust; there is no Node build step for the server.

Build prerequisites:

- Linux: Rust 1.91.1+, Python 3.10+, a C/C++ compiler, linker and the distribution's normal development tools. For example, Debian/Ubuntu uses `build-essential`; RHEL-family hosts use GCC and development tools.
- macOS: Rust, Python 3.10+ and Xcode Command Line Tools.
- Windows: Rust's MSVC toolchain, Python 3.10+ and Visual Studio C++ Build Tools including a Windows SDK. Use `python` in place of `python3` if needed.

To build components separately for packaging or a custom process manager:

```sh
cargo build --release --locked -p holepunch-server -p holepunch-cli
python3 scripts/build-rathole.py
# Install Caddy 2.11.4 separately, or let the native installer obtain it.
```

The executables are `target/release/holepunch-server`, `target/release/holepunch-client` and `apps/desktop/src-tauri/binaries/holepunch-rathole-TARGET` (with `.exe` on Windows). `scripts/prepare-sidecar.sh` remains the compatible Bash entry point for existing build workflows. Use [environment variables and manual systemd templates](DEPLOYMENT.md) if managing these processes yourself.

For production, build as a normal user, [package the result](RELEASE.md), then run the archive installer as root. Avoid compiling unreviewed source as root. Docker is not needed for any of these steps.

## 3. Docker Compose

### Build from the cloned source (available now)

This stack uses Linux host networking so Caddy can access the loopback-only web gateway ports. Docker Desktop is not the production acceptance environment.

```sh
git clone https://github.com/philpm/HolePunch.git
cd HolePunch
python3 scripts/setup.py --interactive
docker compose config --quiet
python3 scripts/compose.py up
docker compose ps
docker compose logs -f center rathole caddy
```

Alternatively, generate configuration without prompts:

```sh
python3 scripts/setup.py --public-host edge.example.com \
  --panel-domain panel.example.com --admin-email admin@example.com
```

`setup.py` writes `.env` once with private credentials. It accepts the same seven `--*-port` / range options as the native installer. The Caddy bootstrap and health check follow the generated custom ports. Setup validates port ranges and overlaps but does not inspect the remote Docker host's listeners; check them before `up`. Compose itself does not change your provider firewall.

`scripts/compose.py up` runs Compose, waits for `/readyz`, and then prints the management URL, initial email/password/API token, configuration path and first-run instructions. `--hide-secrets` redacts the secret values in terminal output. The protected `.env.access.txt` file retains the original access details. It does not print a success message if startup or readiness fails.

Direct `docker compose up -d --build` remains supported. To show access details afterward, run `python3 scripts/access.py --env-file .env`; this read-only command reports credentials, not a health check.

The source Compose path builds the server and patched rathole; Caddy uses its pinned upstream image. It requires no published HolePunch images. Both Compose variants pass the initial administrator credentials to the server and persist application data and certificates in named volumes.

### Use published images (after publication)

Obtain `compose.release.yml` and `bootstrap.Caddyfile` from the same release, together in one directory. Native archives also include `setup.py`, `compose.py`, `deploy.py` and `access.py` at the archive root: run `python3 setup.py --interactive` there to create `.env` locally, then use `python3 compose.py up --pull` after setting the image prefix and version. Generate a protected `.env` with `scripts/setup.py --output /path/to/that/directory/.env`, then add the actual registry prefix and pinned image tag. From that directory:

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

For a checkout, use `python3 scripts/compose.py up --file deploy/compose.release.yml --env-file .env --pull` to pull images, verify readiness and display the same completion summary. Direct `docker compose --env-file .env -f deploy/compose.release.yml ...` remains supported. Keep `bootstrap.Caddyfile` beside the release Compose file because bind paths are relative to it. The current release workflow produces OCI build artifacts for validation; it does not publish a registry automatically.

## First browser sign-in

All three installation paths lead to the same web setup guide. Sign in with the credentials in the completion message; a fresh Self-hosted database opens **Setup guide** automatically. The visible **Language** selector is in the sign-in header and the console header. English is the default; English/Simplified Chinese selection persists per browser and changing it preserves form values.

The guide covers four steps:

1. Confirm the administrator account and display name. Optionally replace the generated password; this requires the current password and signs out all sessions after saving.
2. Confirm the public control address and optional domain suffix. Follow the DNS, public-port and existing-website instructions. The panel domain is inherited from deployment (`HP_PANEL_DOMAIN`); changing it requires a service restart.
3. Check local configuration synchronization and the rathole control listener. These checks do not prove public DNS/firewall reachability or end-to-end tunnel traffic, and inspect saved configuration rather than unsaved drafts.
4. Review the client command, enrollment-code flow and first-tunnel instructions, then save and continue to **Clients**.

Configuration is saved atomically at the final step. **Finish later** keeps setup pending and shows a reminder; unsaved drafts do not survive a page reload. The completion flag is stored in the server database, so a completed installation does not repeat setup in a new browser. Existing databases keep their configuration and can open the guide manually from the sidebar. API tokens/passwords are never returned by the setup status API.

Deployment summaries contain **initial** credentials. A password changed in the console is not written back to `.env`, `config.json` or access summaries. The administrator API token cannot be used as a client token: create a client in the console to get its own one-time enrollment code.

## Next steps and operations

Create a client in the web console, run `holepunch-client connect --server https://panel.example.com` on the machine hosting the local application, and enter its enrollment code. Keep that CLI running; configure tunnels, domains and optional bandwidth limits in the console. See [CLI](CLI.md).

For backups, recovery and monitoring, continue with [operations](DEPLOYMENT.md). Native installations retain all mutable state under `PREFIX/state/`; back up it together with protected `PREFIX/config.json` and the deployed binary version. Stop the stack before a filesystem backup. Never replace a live SQLite database by copying one file.

The installer intentionally handles **fresh installs**, not in-place upgrades. To upgrade, stop the stack, back up `state/` and `config.json`, verify the new archive, replace only the matching binaries under `bin/` and the runner if required, then start and check readiness and real tunnel traffic. Retain ownership/permissions on a systemd installation. Do not rerun fresh setup, regenerate credentials or assume a schema downgrade is safe. Rollback needs the matching data backup. A failed service start leaves the installation and logs available for diagnosis rather than deleting user state.
