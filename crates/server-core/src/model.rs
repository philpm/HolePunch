use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::IpAddr;

pub fn secret() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub fn domain(value: &str) -> Result<String, String> {
    let v = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if v.len() > 253
        || v.parse::<IpAddr>().is_ok()
        || !v.contains('.')
        || v.split('.').any(|s| {
            s.is_empty()
                || s.len() > 63
                || s.starts_with('-')
                || s.ends_with('-')
                || !s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err("Enter a full domain without a scheme, path, port, or wildcard".into());
    }
    Ok(v)
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Tunnel {
    pub id: String,
    pub name: String,
    pub client_id: String,
    pub protocol: String,
    pub local_host: String,
    pub local_port: u16,
    pub public_port: u16,
    pub domain: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing)]
    pub service_token: String,
}
#[derive(Deserialize)]
pub struct TunnelInput {
    pub name: String,
    pub client_id: String,
    pub protocol: String,
    pub local_host: String,
    pub local_port: u16,
    pub public_port: Option<u16>,
    pub domain: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}
fn yes() -> bool {
    true
}
impl TunnelInput {
    pub fn validate(&mut self) -> Result<(), String> {
        self.name = self.name.trim().into();
        if self.name.is_empty() || self.name.len() > 80 {
            return Err("Name must be 1–80 bytes".into());
        }
        if !["tcp", "udp", "web"].contains(&self.protocol.as_str()) {
            return Err("Unsupported tunnel protocol".into());
        }
        if self.local_host.parse::<IpAddr>().is_err() && self.local_host != "localhost" {
            return Err("Local address must be IPv4, IPv6, or localhost".into());
        }
        if self.local_port == 0 {
            return Err("Local port must not be 0".into());
        }
        self.domain = if self.protocol == "web" {
            Some(domain(self.domain.as_deref().unwrap_or(""))?)
        } else {
            None
        };
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub last_seen: i64,
    pub running: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub caddy_url: String,
    pub public_addr: String,
    pub tls_ask_url: String,
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        for s in [&self.caddy_url, &self.tls_ask_url] {
            let u = reqwest::Url::parse(s).map_err(|_| "Invalid URL")?;
            if !["http", "https"].contains(&u.scheme())
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
                || u.query().is_some()
                || u.fragment().is_some()
            {
                return Err(
                    "URL must use HTTP(S) without credentials, a query, or a fragment".into(),
                );
            }
        }
        let u = reqwest::Url::parse(&format!("tcp://{}", self.public_addr))
            .map_err(|_| "Invalid public connection address")?;
        if u.host_str().is_none()
            || u.port().unwrap_or(0) == 0
            || !u.username().is_empty()
            || u.path() != ""
            || u.query().is_some()
            || u.fragment().is_some()
        {
            return Err("Public connection address must be host:port".into());
        }
        Ok(())
    }
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn socket(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// A same-host HTTP application. It never enters a client's rathole configuration.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub enabled: bool,
}
#[derive(Deserialize)]
pub struct SiteInput {
    pub name: String,
    pub domain: String,
    pub upstream_host: String,
    pub upstream_port: u16,
    #[serde(default = "yes")]
    pub enabled: bool,
}
impl SiteInput {
    pub fn validate(&mut self) -> Result<(), String> {
        self.name = self.name.trim().into();
        if self.name.is_empty() || self.name.len() > 80 {
            return Err("Name must be 1–80 bytes".into());
        }
        self.domain = domain(&self.domain)?;
        self.upstream_host = self.upstream_host.trim().to_ascii_lowercase();
        if self.upstream_host == "localhost" {
            self.upstream_host = "127.0.0.1".into();
        }
        if !matches!(self.upstream_host.as_str(), "127.0.0.1" | "::1") {
            return Err("Local applications must listen on 127.0.0.1 or ::1; publish container ports on host loopback".into());
        }
        if self.upstream_port == 0 {
            return Err("Application port must not be 0".into());
        }
        Ok(())
    }
}
