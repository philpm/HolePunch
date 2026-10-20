use crate::model::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub struct NodeConfig {
    pub backends: std::collections::HashMap<String, u16>,
    pub tunnels: Vec<Tunnel>,
    pub sites: Vec<Site>,
    pub settings: Settings,
    pub ask_key: String,
    pub noise_private: String,
    pub bind_addr: String,
    pub panel_domain: Option<String>,
}
#[async_trait]
pub trait NodeManager: Send + Sync {
    async fn sync_config(&self, node_id: &str, config: NodeConfig) -> Result<()>;
}
#[derive(Clone, serde::Serialize)]
pub struct TrafficSample {
    pub timestamp: i64,
    pub upload_bps: u64,
    pub download_bps: u64,
}
#[async_trait]
pub trait TrafficLogger: Send + Sync {
    async fn record(&self, node_id: &str, sample: TrafficSample) -> Result<()>;
}
pub struct NoopTrafficLogger;
#[async_trait]
impl TrafficLogger for NoopTrafficLogger {
    async fn record(&self, _: &str, _: TrafficSample) -> Result<()> {
        Ok(())
    }
}
pub struct LocalNode {
    pub path: PathBuf,
    pub http: reqwest::Client,
}
pub fn server_toml(c: &NodeConfig) -> Result<String> {
    let mut services = serde_json::Map::new();
    for t in &c.tunnels {
        services.insert(t.id.clone(),json!({"type":if t.protocol=="udp"{"udp"}else{"tcp"},"token":t.service_token,"bind_addr":socket(if t.protocol=="web" || c.backends.contains_key(&t.id){"127.0.0.1"}else{"0.0.0.0"},*c.backends.get(&t.id).unwrap_or(&t.public_port))}));
    }
    let v = json!({"server":{"bind_addr":c.bind_addr,"transport":{"type":"noise","noise":{"local_private_key":c.noise_private}},"services":services}});
    Ok(toml::to_string_pretty(&v)?)
}
pub fn caddy_config(c: &NodeConfig) -> Result<Value> {
    let mut ask = reqwest::Url::parse(&c.settings.tls_ask_url)?;
    ask.query_pairs_mut().append_pair("key", &c.ask_key);
    let mut routes:Vec<Value>=c.tunnels.iter().filter_map(|t|t.domain.as_ref().map(|d|json!({"@id":format!("hp-{}",t.id),"match":[{"host":[d]}],"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":format!("127.0.0.1:{}",t.public_port)}]}],"terminal":true}))).collect();
    for site in c.sites.iter().filter(|s| s.enabled) {
        routes.push(json!({"@id":format!("site-{}",site.id),"match":[{"host":[site.domain]}],"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":socket(&site.upstream_host,site.upstream_port)}]}],"terminal":true}));
    }
    if let Some(d) = &c.panel_domain {
        let mut panel: std::net::SocketAddr = std::env::var("HP_BIND")
            .unwrap_or("127.0.0.1:3000".into())
            .parse()
            .context("HP_BIND must be a socket address")?;
        if panel.ip().is_unspecified() {
            panel.set_ip(if panel.is_ipv4() {
                std::net::Ipv4Addr::LOCALHOST.into()
            } else {
                std::net::Ipv6Addr::LOCALHOST.into()
            });
        }

        routes.push(json!({"match":[{"host":[d]}],"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":panel.to_string()}]}],"terminal":true}));
    }
    routes.push(json!({"handle":[{"handler":"static_response","status_code":404}]}));
    let mut config = json!({"admin":{"listen":std::env::var("HP_CADDY_LISTEN").unwrap_or("127.0.0.1:2019".into())},"apps":{"http":{"http_port":std::env::var("HP_HTTP_PORT").ok().and_then(|p|p.parse::<u16>().ok()).unwrap_or(80),"servers":{"holepunch":{"listen":[std::env::var("HP_HTTPS_LISTEN").unwrap_or(":443".into())],"routes":routes,"tls_connection_policies":[{}]}}},"tls":{"automation":{"policies":[{"on_demand":true}],"on_demand":{"permission":{"module":"http","endpoint":ask.as_str()}}}}}});
    // Explicit development mode: never install a test CA into the host trust store.
    if std::env::var("HP_TLS_INTERNAL").as_deref() == Ok("1") {
        config["apps"]["pki"] =
            json!({"certificate_authorities":{"local":{"install_trust":false}}});
        config["apps"]["tls"]["automation"]["policies"][0]["issuers"] =
            json!([{"module":"internal"}]);
    }
    Ok(config)
}
pub fn private_write(path: &Path, data: &str) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().context("config has no parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension("tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(data.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
#[async_trait]
impl NodeManager for LocalNode {
    async fn sync_config(&self, _: &str, c: NodeConfig) -> Result<()> {
        let config = server_toml(&c)?;
        if std::fs::read_to_string(&self.path).ok().as_deref() != Some(&config) {
            private_write(&self.path, &config)?;
        }
        let url = format!("{}/config/", c.settings.caddy_url.trim_end_matches('/'));
        let desired = caddy_config(&c)?;
        let current: Value = self
            .http
            .get(&url)
            .send()
            .await
            .context("Caddy unavailable")?
            .error_for_status()
            .context("Caddy configuration read failed")?
            .json()
            .await?;
        if current == desired {
            return Ok(());
        }
        self.http
            .post(url)
            .json(&desired)
            .send()
            .await
            .context("Caddy unavailable")?
            .error_for_status()
            .context("Caddy rejected configuration")?;
        Ok(())
    }
}
