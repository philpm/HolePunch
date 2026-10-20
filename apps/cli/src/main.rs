use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use holepunch_client::{Binding, Client};
use std::{
    fs::OpenOptions,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
#[derive(Parser)]
#[command(
    name = "holepunch-client",
    version,
    about = "HolePunch CLI: configure tunnels on the web, connect and run locally"
)]
struct Args {
    /// Client data directory (default: HP_CLIENT_DATA or ~/.holepunch).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Enroll a device or run its saved binding; Ctrl-C stops forwarding.
    Connect {
        /// Control center URL; HTTPS is required except on loopback.
        #[arg(long)]
        server: Option<String>,
        /// One-time enrollment code (prefer the interactive prompt or --token-stdin).
        #[arg(long)]
        enroll_token: Option<String>,
        /// Read the one-time enrollment code from standard input.
        #[arg(long)]
        token_stdin: bool,
    },
    /// Show process, binding, and synchronization status without credentials.
    Status,
    /// Read the bounded rathole data-plane log.
    Logs {
        /// Keep reading as new log entries arrive.
        #[arg(long)]
        follow: bool,
    },
    /// Check authorization, control connectivity, and local TCP services.
    Doctor,
    /// Stop the current CLI process while retaining its binding.
    Disconnect,
    /// Stop the client and remove its local binding and generated configuration.
    Logout,
    /// Print the client version.
    Version,
    /// Manage a macOS LaunchAgent or Linux user systemd service.
    Service {
        /// Install, start, stop, or uninstall the background service.
        #[arg(value_parser=["install","start","stop","uninstall"])]
        action: String,
    },
}
fn directory(arg: Option<PathBuf>) -> PathBuf {
    arg.or_else(|| std::env::var_os("HP_CLIENT_DATA").map(PathBuf::from))
        .unwrap_or_else(|| {
            if std::env::var_os("HP_CLIENT_TOKEN").is_some() {
                PathBuf::from("data/client")
            } else {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
                    .join(".holepunch")
            }
        })
}
fn private(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().context("invalid path")?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    std::fs::write(path, text)?;
    Ok(())
}
fn binary() -> PathBuf {
    std::env::var_os("HP_RATHOLE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .with_file_name(if cfg!(windows) {
                    "rathole.exe"
                } else {
                    "rathole"
                })
        })
}
fn validate_server(s: &str) -> Result<()> {
    let u = reqwest::Url::parse(s)?;
    anyhow::ensure!(
        u.scheme() == "https"
            || (u.scheme() == "http"
                && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
        "Public control centers must use HTTPS"
    );
    anyhow::ensure!(
        u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none(),
        "URLs must not contain credentials, queries, or fragments"
    );
    Ok(())
}
fn lock(dir: &Path) -> Result<std::fs::File> {
    private_dir(dir)?;
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("process.lock"))?;
    fs2::FileExt::try_lock_exclusive(&f)
        .context("The client is already running; use status or disconnect")?;
    Ok(f)
}
fn private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let dir = directory(args.data_dir);
    let command = args.command.unwrap_or(Command::Connect {
        server: None,
        enroll_token: None,
        token_stdin: false,
    });
    match command {
        Command::Version => println!("holepunch-client {}", env!("CARGO_PKG_VERSION")),
        Command::Status => {
            let running = lock(&dir).is_err();
            println!(
                "Process status: {}",
                if running { "Running" } else { "Stopped" }
            );
            if running {
                println!(
                    "{}",
                    std::fs::read_to_string(dir.join("status.json"))
                        .unwrap_or_else(|_| "Waiting for synchronization".into())
                );
            }
            println!("Bound: {}", dir.join("binding.json").exists());
        }
        Command::Disconnect | Command::Logout => {
            let logout = matches!(command, Command::Logout);
            if lock(&dir).is_err() {
                private(&dir.join("control"), if logout { "logout" } else { "stop" })?;
                for _ in 0..50 {
                    if lock(&dir).is_ok() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                anyhow::ensure!(lock(&dir).is_ok(), "Stop was not confirmed; check the logs");
            }
            if logout {
                for name in ["binding.json", "client.toml", "status.json"] {
                    let _ = std::fs::remove_file(dir.join(name));
                }
            }
            println!(
                "{}",
                if logout {
                    "Stopped and removed the binding"
                } else {
                    "Stopped; binding retained"
                }
            );
        }
        Command::Logs { follow } => {
            let mut offset = None;
            loop {
                let path = dir.join("rathole.log");
                match std::fs::File::open(&path) {
                    Ok(mut file) => {
                        let len = file.metadata()?.len();
                        let start = offset.filter(|p| *p <= len).unwrap_or_else(|| {
                            if offset.is_some() {
                                0
                            } else {
                                len.saturating_sub(32768)
                            }
                        });
                        file.seek(SeekFrom::Start(start))?;
                        let mut bytes = Vec::new();
                        file.take(32768).read_to_end(&mut bytes)?;
                        offset = Some(start + bytes.len() as u64);
                        print!("{}", String::from_utf8_lossy(&bytes));
                        io::stdout().flush()?;
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        if offset.is_none() {
                            println!("No logs yet");
                            offset = Some(0);
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
                if !follow {
                    break;
                }
                tokio::select! {_=tokio::signal::ctrl_c()=>break,_=tokio::time::sleep(std::time::Duration::from_secs(1))=>{}}
            }
        }
        Command::Doctor => {
            println!("Data directory: {}", dir.display());
            println!(
                "Data-plane binary: {} ({})",
                binary().display(),
                binary().exists()
            );
            let b: Binding = serde_json::from_str(
                &std::fs::read_to_string(dir.join("binding.json"))
                    .context("Not bound; run connect --server URL first")?,
            )?;
            validate_server(&b.api_url)?;
            let http = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let snapshot: holepunch_client::Snapshot = http
                .get(format!(
                    "{}/api/v1/client/config",
                    b.api_url.trim_end_matches('/')
                ))
                .bearer_auth(&b.token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            println!("HTTPS/API/device authorization: passed");
            let config: toml::Value = toml::from_str(&snapshot.config)?;
            if let Some(remote) = config
                .get("client")
                .and_then(|c| c.get("remote_addr"))
                .and_then(|v| v.as_str())
            {
                let ok = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    tokio::net::TcpStream::connect(remote),
                )
                .await
                .is_ok_and(|r| r.is_ok());
                println!(
                    "Data control port: {}",
                    if ok { "Reachable" } else { "Unreachable" }
                );
            }
            for t in snapshot.tunnels {
                let host = t["local_host"].as_str().unwrap_or("127.0.0.1");
                let port = t["local_port"].as_u64().unwrap_or(0);
                if t["protocol"] == "udp" {
                    println!(
                        "{}: UDP requires a real application request to verify",
                        t["name"]
                    );
                } else {
                    let ok = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        tokio::net::TcpStream::connect((host, port as u16)),
                    )
                    .await
                    .is_ok_and(|r| r.is_ok());
                    println!(
                        "{}: local TCP {}",
                        t["name"],
                        if ok { "Reachable" } else { "Unreachable" }
                    );
                }
            }
        }
        Command::Service { action } => service(&dir, &action).await?,
        Command::Connect {
            server,
            enroll_token,
            token_stdin,
        } => {
            let _lock = lock(&dir)?;
            let _ = std::fs::remove_file(dir.join("control"));
            let binding = if let Some(server) = server {
                validate_server(&server)?;
                anyhow::ensure!(
                    !dir.join("binding.json").exists(),
                    "Already bound; run logout before rebinding"
                );
                let token = if let Some(t) = enroll_token {
                    t
                } else {
                    if !token_stdin {
                        eprint!("Enter the one-time enrollment code: ");
                        io::stderr().flush()?;
                    }
                    let mut t = String::new();
                    io::stdin().read_line(&mut t)?;
                    t.trim().into()
                };
                let http = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?;
                let res = http
                    .post(format!("{}/api/v1/enroll", server.trim_end_matches('/')))
                    .json(&serde_json::json!({"token":token}))
                    .send()
                    .await?;
                anyhow::ensure!(
                    res.status().is_success(),
                    "Enrollment failed ({}); generate a new code in the web console",
                    res.status()
                );
                let data: serde_json::Value = res.json().await?;
                Binding {
                    api_url: server,
                    token: data["token"]
                        .as_str()
                        .context("Invalid enrollment response")?
                        .into(),
                }
            } else if let (Ok(api_url), Ok(token)) = (
                std::env::var("HP_API_URL"),
                std::env::var("HP_CLIENT_TOKEN"),
            ) {
                Binding { api_url, token }
            } else {
                serde_json::from_str(&std::fs::read_to_string(dir.join("binding.json")).context(
                    "First use: holepunch-client connect --server https://panel.example.com",
                )?)?
            };
            let mut client = Client::new(dir.clone(), binary())?;
            client.bind(binding.clone()).await?;
            private(&dir.join("binding.json"), &serde_json::to_string(&binding)?)?;
            client.start().await?;
            println!("Device connected and running. Configure tunnels in the web console; press Ctrl-C to stop.");
            #[cfg(unix)]
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let shutdown = async {
                #[cfg(unix)]
                {
                    tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }
            };
            tokio::pin!(shutdown);
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut n = 0;
            let mut logout = false;
            loop {
                tokio::select! {_=&mut shutdown=>break,_=tick.tick()=>{
                if let Ok(cmd)=std::fs::read_to_string(dir.join("control")){logout=cmd=="logout";break;}
                if n%10==0{if let Err(e)=client.tick().await{eprintln!("{e:#}");}}n+=1;
                let status=client.status();if status.connection=="revoked"{logout=true;break;}private(&dir.join("status.json"),&serde_json::to_string_pretty(&status)?)?;
                }}
            }
            client.stop().await?;
            if logout {
                let _ = std::fs::remove_file(dir.join("binding.json"));
                let _ = std::fs::remove_file(dir.join("client.toml"));
            }
            let _ = std::fs::remove_file(dir.join("control"));
            let _ = std::fs::remove_file(dir.join("status.json"));
        }
    }
    Ok(())
}
async fn service(dir: &Path, action: &str) -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = std::path::absolute(dir)?;
    let sidecar = std::path::absolute(binary())?;
    if action == "install" {
        anyhow::ensure!(
            dir.join("binding.json").is_file(),
            "Run connect to bind the device before installing the background service"
        );
        anyhow::ensure!(sidecar.is_file(), "rathole data-plane binary not found");
    }
    #[cfg(target_os = "linux")]
    {
        let base = PathBuf::from(std::env::var("HOME")?).join(".config/systemd/user");
        let path = base.join("holepunch-client.service");
        if action == "install" {
            let quote = |p: &Path| {
                p.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('%', "%%")
            };
            private(&path,&format!("[Unit]\nDescription=HolePunch client\nAfter=network-online.target\n[Service]\nEnvironment=\"HP_RATHOLE_BIN={}\"\nExecStart=:\"{}\" --data-dir \"{}\" connect\nRestart=on-failure\nRestartSec=10\n[Install]\nWantedBy=default.target\n",quote(&sidecar),quote(&exe),quote(&dir)))?;
            run("systemctl", &["--user", "daemon-reload"]).await?;
            run("systemctl", &["--user", "enable", "holepunch-client"]).await?;
            println!("User service installed; run service start. Enable lingering to keep it running after logout.");
        } else if action == "uninstall" {
            if path.exists() {
                run(
                    "systemctl",
                    &["--user", "disable", "--now", "holepunch-client"],
                )
                .await?;
                std::fs::remove_file(path)?;
            }
            run("systemctl", &["--user", "daemon-reload"]).await?;
        } else {
            run("systemctl", &["--user", action, "holepunch-client"]).await?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let base = PathBuf::from(std::env::var("HOME")?).join("Library/LaunchAgents");
        let path = base.join("dev.holepunch.cli.plist");
        let uid = String::from_utf8(
            tokio::process::Command::new("id")
                .arg("-u")
                .output()
                .await?
                .stdout,
        )?
        .trim()
        .to_owned();
        let domain = format!("gui/{uid}");
        let target = format!("{domain}/dev.holepunch.cli");
        if action == "install" {
            let escape = |s: &str| {
                s.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
            };
            private(&path,&format!("<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>dev.holepunch.cli</string><key>ProgramArguments</key><array><string>{}</string><string>--data-dir</string><string>{}</string><string>connect</string></array><key>EnvironmentVariables</key><dict><key>HP_RATHOLE_BIN</key><string>{}</string></dict><key>RunAtLoad</key><true/><key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict><key>ThrottleInterval</key><integer>10</integer></dict></plist>",escape(&exe.to_string_lossy()),escape(&dir.to_string_lossy()),escape(&sidecar.to_string_lossy())))?;
            println!("Login service installed; run service start.");
        } else if action == "start" {
            let loaded = tokio::process::Command::new("launchctl")
                .args(["print", &target])
                .output()
                .await?
                .status
                .success();
            if loaded {
                run("launchctl", &["kickstart", &target]).await?;
            } else {
                run(
                    "launchctl",
                    &["bootstrap", &domain, &path.to_string_lossy()],
                )
                .await?;
            }
        } else {
            let loaded = tokio::process::Command::new("launchctl")
                .args(["print", &target])
                .output()
                .await?
                .status
                .success();
            if loaded {
                run("launchctl", &["bootout", &target]).await?;
            }
            if action == "uninstall" && path.exists() {
                std::fs::remove_file(path)?;
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    anyhow::bail!("Background service installation is not supported on this platform; run connect in the foreground");
    Ok(())
}
async fn run(program: &str, args: &[&str]) -> Result<()> {
    anyhow::ensure!(
        tokio::process::Command::new(program)
            .args(args)
            .status()
            .await?
            .success(),
        "Service management command failed"
    );
    Ok(())
}
