mod logging;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio};
#[derive(Clone, Serialize, Deserialize)]
pub struct Binding {
    pub api_url: String,
    pub token: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub client_id: String,
    pub tunnels: Vec<serde_json::Value>,
    pub config: String,
    pub revision: Option<String>,
}
#[derive(Serialize)]
pub struct Status {
    pub running: bool,
    pub bound: bool,
    pub tunnels: Vec<serde_json::Value>,
    pub error: Option<String>,
    pub desired_running: bool,
    pub connection: String,
    pub last_sync_age_seconds: Option<u64>,
}
pub struct Client {
    binding: Option<Binding>,
    http: reqwest::Client,
    child: Option<tokio::process::Child>,
    pub data_dir: PathBuf,
    pub binary: PathBuf,
    snapshot: Option<Snapshot>,
    pub error: Option<String>,
    desired_running: bool,
    last_sync: Option<std::time::Instant>,
    revoked: bool,
}
impl Client {
    pub fn new(data_dir: PathBuf, binary: PathBuf) -> Result<Self> {
        Ok(Self {
            binding: None,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            child: None,
            data_dir,
            binary,
            snapshot: None,
            error: None,
            desired_running: false,
            last_sync: None,
            revoked: false,
        })
    }
    pub async fn bind(&mut self, binding: Binding) -> Result<()> {
        let u = reqwest::Url::parse(&binding.api_url)?;
        anyhow::ensure!(
            u.scheme() == "https"
                || (u.scheme() == "http"
                    && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
            "Public APIs must use HTTPS; HTTP is allowed only on localhost"
        );
        anyhow::ensure!(
            u.query().is_none()
                && u.fragment().is_none()
                && u.username().is_empty()
                && u.password().is_none(),
            "API URLs must not contain credentials, query parameters, or fragments"
        );
        anyhow::ensure!(binding.token.len() >= 32, "Invalid client token format");
        let snapshot = self.fetch(&binding).await?;
        let _: toml::Value =
            toml::from_str(&snapshot.config).context("The server returned invalid TOML")?;
        self.stop().await?;
        self.save(&snapshot.config)?;
        self.binding = Some(binding);
        self.snapshot = Some(snapshot);
        self.last_sync = Some(std::time::Instant::now());
        self.revoked = false;
        self.error = None;
        Ok(())
    }
    async fn fetch(&self, b: &Binding) -> Result<Snapshot> {
        let r = self
            .http
            .get(format!(
                "{}/api/v1/client/config",
                b.api_url.trim_end_matches('/')
            ))
            .bearer_auth(&b.token)
            .send()
            .await?;
        if r.status().as_u16() == 403 {
            return Err(Suspended.into());
        }
        if r.status().as_u16() == 401 {
            return Err(Revoked.into());
        }
        anyhow::ensure!(
            r.status().is_success(),
            "Configuration synchronization failed (HTTP {}); check the access token",
            r.status()
        );
        anyhow::ensure!(
            r.content_length().unwrap_or(0) <= 2 * 1024 * 1024,
            "Configuration response is too large"
        );
        let mut response = r;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            anyhow::ensure!(
                body.len() + chunk.len() <= 2 * 1024 * 1024,
                "Configuration response is too large"
            );
            body.extend_from_slice(&chunk);
        }
        let snapshot: Snapshot = serde_json::from_slice(&body)?;
        let _: toml::Value =
            toml::from_str(&snapshot.config).context("The server returned invalid TOML")?;
        Ok(snapshot)
    }
    fn save(&self, text: &str) -> Result<()> {
        let _: toml::Value = toml::from_str(text).context("The server returned invalid TOML")?;
        std::fs::create_dir_all(&self.data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let path = self.data_dir.join("client.toml");
        if std::fs::read_to_string(&path).ok().as_deref() == Some(text) {
            return Ok(());
        }
        use std::io::Write;
        let tmp = self.data_dir.join("client.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }
    pub async fn sync(&mut self) -> Result<()> {
        let binding = self.binding.as_ref().context("Bind the client first")?;
        match self.fetch(binding).await {
            Ok(s) => {
                self.save(&s.config)?;
                self.snapshot = Some(s);
                self.last_sync = Some(std::time::Instant::now());
                self.revoked = false;
                self.error = None;
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.to_string());
                if e.downcast_ref::<Revoked>().is_some() {
                    self.revoked = true;
                    self.desired_running = false;
                    self.terminate().await?;
                    self.snapshot = None;
                    self.last_sync = None;
                    let _ = std::fs::remove_file(self.data_dir.join("client.toml"));
                } else if e.downcast_ref::<Suspended>().is_some()
                    || self.last_sync.is_none_or(|t| t.elapsed().as_secs() >= 90)
                {
                    // A bounded authorization lease prevents indefinite use of stale credentials.
                    // Preserve the user's intent so that a successful future sync can reconnect.
                    self.terminate().await?;
                }
                Err(e)
            }
        }
    }
    pub fn running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => return true,
                Ok(Some(status)) => {
                    self.error = Some(format!("rathole exited: {status}; check rathole.log"));
                }
                Err(e) => self.error = Some(e.to_string()),
            }
            self.child = None;
        }
        false
    }
    pub fn status(&mut self) -> Status {
        Status {
            running: self.running(),
            bound: self.binding.is_some(),
            tunnels: self
                .snapshot
                .as_ref()
                .map(|s| s.tunnels.clone())
                .unwrap_or_default(),
            error: self.error.clone(),
            desired_running: self.desired_running,
            connection: if self.revoked {
                "revoked"
            } else if self.binding.is_none() {
                "unbound"
            } else if self.error.is_some() {
                "retrying"
            } else {
                "synced"
            }
            .into(),
            last_sync_age_seconds: self.last_sync.map(|t| t.elapsed().as_secs()),
        }
    }
    pub async fn start(&mut self) -> Result<()> {
        self.sync().await?;
        if self.running() {
            return Ok(());
        }
        let log = std::sync::Arc::new(std::sync::Mutex::new(logging::RollingLog::new(
            self.data_dir.join("rathole.log"),
        )?));
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .arg("--client")
            .arg(self.data_dir.join("client.toml"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command
            .spawn()
            .context("Cannot start rathole; check the binary path")?;
        logging::pump(child.stdout.take().unwrap(), log.clone());
        logging::pump(child.stderr.take().unwrap(), log);
        self.child = Some(child);
        self.desired_running = true;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        anyhow::ensure!(self.running(), "rathole failed to start; check rathole.log");
        if let Err(e) = self.ping().await {
            self.error = Some(e.to_string());
        }
        Ok(())
    }
    pub async fn stop(&mut self) -> Result<()> {
        self.desired_running = false;
        self.terminate().await?;
        if self.binding.is_some() {
            let _ = self.ping().await;
        }
        Ok(())
    }
    async fn terminate(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().await?;
            }
            let _ = child.wait().await;
        }
        Ok(())
    }
    pub async fn forget(&mut self) -> Result<()> {
        self.stop().await?;
        match std::fs::remove_file(self.data_dir.join("client.toml")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        self.binding = None;
        self.snapshot = None;
        self.last_sync = None;
        self.revoked = false;
        self.error = None;
        Ok(())
    }
    pub fn log_tail(&self) -> Result<String> {
        use std::io::{Read, Seek, SeekFrom};
        let path = self.data_dir.join("rathole.log");
        if !path.exists() {
            return Ok("No logs yet".into());
        }
        let mut f = std::fs::File::open(path)?;
        let length = f.metadata()?.len();
        f.seek(SeekFrom::Start(length.saturating_sub(32768)))?;
        let mut bytes = Vec::new();
        f.take(32768).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    pub async fn ping(&mut self) -> Result<()> {
        let running = self.running();
        let b = self.binding.as_ref().context("Not bound")?;
        self.http
            .post(format!(
                "{}/api/v1/client/heartbeat",
                b.api_url.trim_end_matches('/')
            ))
            .bearer_auth(&b.token)
            .json(&serde_json::json!({"running":running}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
    pub async fn tick(&mut self) -> Result<()> {
        if self.binding.is_none() || self.revoked {
            return Ok(());
        }
        self.sync().await?;
        if self.desired_running && !self.running() {
            self.start().await?;
        }
        self.ping().await
    }
}

#[derive(Debug)]
struct Suspended;
impl std::fmt::Display for Suspended {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Account authorization is suspended; waiting for an administrator to restore it"
        )
    }
}
impl std::error::Error for Suspended {}
#[derive(Debug)]
struct Revoked;
impl std::fmt::Display for Revoked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The client token has been revoked or expired; bind the device again"
        )
    }
}
impl std::error::Error for Revoked {}

#[cfg(all(test, unix))]
mod resilience_tests {
    use super::*;
    use axum::{
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    };
    #[tokio::test]
    async fn lease_recovery_revocation_and_explicit_stop() {
        let status = Arc::new(AtomicU16::new(200));
        let responder = status.clone();
        let server=Router::new().route("/api/v1/client/config",get(move||{let state=responder.clone();async move{
            let code=state.load(Ordering::SeqCst);if code!=200{return StatusCode::from_u16(code).unwrap().into_response();}
            Json(serde_json::json!({"client_id":"test","tunnels":[],"config":"[client]\nremote_addr='127.0.0.1:2333'","revision":"1"})).into_response()
        }})).route("/api/v1/client/heartbeat",post(||async{StatusCode::OK}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("fake-rathole");
        std::fs::write(&binary, "#!/bin/sh\nexec sleep 60\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut client = Client::new(dir.path().join("state"), binary).unwrap();
        client
            .bind(Binding {
                api_url: format!("http://{addr}"),
                token: "test".repeat(16),
            })
            .await
            .unwrap();
        client.start().await.unwrap();
        status.store(503, Ordering::SeqCst);
        assert!(client.tick().await.is_err());
        assert!(client.running());
        assert!(client.desired_running);
        client.last_sync = Some(std::time::Instant::now() - std::time::Duration::from_secs(91));
        assert!(client.tick().await.is_err());
        assert!(!client.running());
        assert!(client.desired_running);
        status.store(200, Ordering::SeqCst);
        client.tick().await.unwrap();
        assert!(client.running());
        client.stop().await.unwrap();
        client.tick().await.unwrap();
        assert!(!client.running());
        client.start().await.unwrap();
        status.store(403, Ordering::SeqCst);
        assert!(client.tick().await.is_err());
        assert!(!client.running());
        assert!(client.desired_running);
        status.store(200, Ordering::SeqCst);
        client.tick().await.unwrap();
        assert!(client.running());
        status.store(401, Ordering::SeqCst);
        assert!(client.tick().await.is_err());
        assert!(!client.running());
        assert!(!client.desired_running);
        assert_eq!(client.status().connection, "revoked");
        assert!(!client.data_dir.join("client.toml").exists());
        status.store(200, Ordering::SeqCst);
        client.tick().await.unwrap();
        assert!(!client.running());
        client.forget().await.unwrap();
        assert!(!client.status().bound);
        task.abort();
    }
}
