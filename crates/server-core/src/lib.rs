pub mod bandwidth;
pub mod edition;
pub mod maintenance;
pub mod model;
pub mod node;
pub mod service;
mod setup;
pub mod store;
pub mod tenant;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use model::*;
use node::*;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use store::Store;
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct App {
    pub edition: edition::Edition,
    pub store: Arc<Mutex<Store>>,
    pub admin_hash: String,
    pub node: Arc<dyn NodeManager>,
    pub sync: Arc<tokio::sync::Mutex<()>>,
    pub metrics: Arc<Mutex<Metrics>>,
    pub bind_addr: String,
    pub panel_domain: Option<String>,
    pub port_start: u16,
    pub port_end: u16,
    pub gateway: Arc<tokio::sync::Mutex<bandwidth::Gateway>>,
    pub last_sync: Arc<Mutex<Option<std::time::Instant>>>,
}
#[derive(Default)]
pub struct Metrics {
    pub cpu: f32,
    pub memory: u64,
    pub total_memory: u64,
    pub process_memory: u64,
    pub samples: VecDeque<TrafficSample>,
}
#[derive(Debug)]
pub struct Error(pub StatusCode, pub String);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error=%e,"request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Server operation failed; check the logs".into(),
        )
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        anyhow::Error::from(e).into()
    }
}
pub type ApiResult<T> = Result<T, Error>;
pub fn bad(s: impl Into<String>) -> Error {
    Error(StatusCode::BAD_REQUEST, s.into())
}
fn notfound() -> Error {
    Error(StatusCode::NOT_FOUND, "Record not found".into())
}
fn bearer(h: &HeaderMap) -> Option<&str> {
    h.get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}
fn admin(a: &App, h: &HeaderMap) -> ApiResult<()> {
    if h.contains_key(header::COOKIE) {
        let u = tenant::identity(a, h, false)?;
        return if u.role == "admin" {
            Ok(())
        } else {
            Err(Error(
                StatusCode::FORBIDDEN,
                "Administrator access required".into(),
            ))
        };
    }
    let multi: bool = a.store.lock().unwrap().db.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE role='admin')",
        [],
        |r| r.get(0),
    )?;
    if multi && std::env::var("HP_LEGACY_ADMIN").as_deref() != Ok("1") {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Sign in to the administrator account with email and password".into(),
        ));
    }
    if bearer(h)
        .map(|s| bool::from(hash(s).as_bytes().ct_eq(a.admin_hash.as_bytes())))
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid administrator token".into(),
        ))
    }
}
fn client(a: &App, h: &HeaderMap) -> ApiResult<String> {
    let raw =
        bearer(h).ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Missing client token".into()))?;
    use rusqlite::OptionalExtension;
    let id: Option<String> = a
        .store
        .lock()
        .unwrap()
        .db
        .query_row(
            "SELECT id FROM clients WHERE token_hash=? AND active=1",
            [hash(raw)],
            |r| r.get(0),
        )
        .optional()?;
    let id = id.ok_or_else(|| {
        Error(
            StatusCode::UNAUTHORIZED,
            "Client token expired or revoked".into(),
        )
    })?;
    if !tenant::owner_active(&a.store.lock().unwrap(), &id) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "The device owner account is unavailable".into(),
        ));
    }
    Ok(id)
}
pub fn dirty(s: &Store) -> anyhow::Result<()> {
    let rev = s
        .get("revision")?
        .unwrap_or_default()
        .parse::<u64>()
        .unwrap_or(0)
        + 1;
    s.set("revision", &rev.to_string())?;
    Ok(())
}
impl App {
    pub fn new(
        path: PathBuf,
        admin_token: &str,
        node: Arc<dyn NodeManager>,
    ) -> anyhow::Result<Self> {
        Self::new_with_edition(path, admin_token, node, edition::Edition::Cloud)
    }
    pub fn new_with_edition(
        path: PathBuf,
        admin_token: &str,
        node: Arc<dyn NodeManager>,
        edition: edition::Edition,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            admin_token.len() >= 32,
            "HP_ADMIN_TOKEN must contain at least 32 characters"
        );
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let s = Store::open(&path)?;
        edition::configure(&s, edition)?;
        tenant::bootstrap(&s)?;
        if edition == edition::Edition::SelfHosted {
            let admins: i64 = s.db.query_row(
                "SELECT COUNT(*) FROM users WHERE role='admin' AND status='active'",
                [],
                |r| r.get(0),
            )?;
            anyhow::ensure!(admins==1,"Set HP_BOOTSTRAP_EMAIL and HP_BOOTSTRAP_PASSWORD (12+ bytes) for the self-hosted administrator; SMTP is not required");
        }
        if s.get("config")?.is_none() {
            let config = Settings {
                caddy_url: std::env::var("HP_CADDY_URL").unwrap_or("http://127.0.0.1:2019".into()),
                public_addr: std::env::var("HP_PUBLIC_ADDR").unwrap_or("127.0.0.1:2333".into()),
                tls_ask_url: std::env::var("HP_TLS_ASK_URL")
                    .unwrap_or("http://127.0.0.1:3000/api/v1/tls/check".into()),
            };
            config.validate().map_err(anyhow::Error::msg)?;
            s.set("config", &serde_json::to_string(&config)?)?;
        }
        for key in ["ask_key", "noise_private"] {
            if s.get(key)?.is_none() {
                let v = if key == "noise_private" {
                    use base64::Engine;
                    use rand::RngCore;
                    let mut bytes = [0; 32];
                    rand::thread_rng().fill_bytes(&mut bytes);
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                } else {
                    secret()
                };
                s.set(key, &v)?;
            }
        }
        s.db.execute("UPDATE clients SET last_seen=0,running=0", [])?;
        dirty(&s)?;
        let panel_domain = std::env::var("HP_PANEL_DOMAIN")
            .ok()
            .filter(|d| !d.is_empty())
            .map(|d| domain(&d).map_err(anyhow::Error::msg))
            .transpose()?;
        anyhow::ensure!(
            !panel_domain.as_ref().is_some_and(|d| s
                .tunnels()
                .map(|ts| ts.iter().any(|t| t.domain.as_ref() == Some(d)))
                .unwrap_or(true)),
            "Panel domain conflicts with a stored tunnel; choose another panel domain"
        );
        anyhow::ensure!(
            !panel_domain.as_ref().is_some_and(|d| s
                .sites()
                .map(|sites| sites.iter().any(|site| &site.domain == d))
                .unwrap_or(true)),
            "Panel domain conflicts with a stored site; choose another panel domain"
        );
        let port_start = std::env::var("HP_PORT_START")
            .unwrap_or("20000".into())
            .parse()?;
        let port_end = std::env::var("HP_PORT_END")
            .unwrap_or("20100".into())
            .parse()?;
        anyhow::ensure!(
            port_start >= 1024 && port_end >= port_start,
            "invalid port range"
        );
        Ok(Self {
            edition,
            store: Arc::new(Mutex::new(s)),
            admin_hash: hash(admin_token),
            node,
            sync: Arc::new(tokio::sync::Mutex::new(())),
            metrics: Arc::new(Mutex::new(Metrics::default())),
            bind_addr: std::env::var("HP_RATHOLE_BIND").unwrap_or("0.0.0.0:2333".into()),
            panel_domain,
            port_start,
            port_end,
            gateway: Arc::new(tokio::sync::Mutex::new(bandwidth::Gateway::default())),
            last_sync: Arc::new(Mutex::new(None)),
        })
    }
    pub async fn reconcile(&self) -> anyhow::Result<()> {
        let _guard = self.sync.lock().await;
        let (revision, config, specs) = {
            let s = self.store.lock().unwrap();
            let revision = s.get("revision")?.unwrap_or_default();
            let mut backends = std::collections::HashMap::new();
            let mut specs = vec![];
            for t in s.active_tunnels()? {
                use rusqlite::OptionalExtension;
                if let Some((bps, port)) =
                    s.db.query_row(
                        "SELECT bps,backend_port FROM tunnel_limits WHERE tunnel_id=?",
                        [&t.id],
                        |r| Ok((r.get::<_, u64>(0)?, r.get::<_, u16>(1)?)),
                    )
                    .optional()?
                {
                    let unlimited: bool = s.db.query_row(
                        "SELECT EXISTS(SELECT 1 FROM tunnel_unlimited WHERE tunnel_id=?)",
                        [&t.id],
                        |r| r.get(0),
                    )?;
                    if bps == 0 && !unlimited {
                        continue;
                    }
                    anyhow::ensure!(
                        !unlimited || self.edition == edition::Edition::SelfHosted,
                        "Cloud cannot forward an unlimited tunnel"
                    );
                    backends.insert(t.id.clone(), port);
                    specs.push(bandwidth::Spec {
                        id: t.id.clone(),
                        bind: format!(
                            "{}:{}",
                            if t.protocol == "web" {
                                "127.0.0.1"
                            } else {
                                "0.0.0.0"
                            },
                            t.public_port
                        )
                        .parse()?,
                        backend: format!("127.0.0.1:{port}").parse()?,
                        bps: if unlimited { None } else { Some(bps) },
                        udp: t.protocol == "udp",
                    });
                }
            }
            (
                revision,
                NodeConfig {
                    backends,
                    tunnels: s.active_tunnels()?,
                    sites: s.sites()?,
                    settings: s.settings()?,
                    ask_key: s.get("ask_key")?.unwrap(),
                    noise_private: s.get("noise_private")?.unwrap(),
                    bind_addr: self.bind_addr.clone(),
                    panel_domain: self.panel_domain.clone(),
                },
                specs,
            )
        };
        // Remove changed/obsolete public gateways before applying any new routes.
        let mut gateway = self.gateway.lock().await;
        let result = async {
            gateway.apply(specs).await?;
            self.node.sync_config("local", config).await
        }
        .await;
        drop(gateway);
        let s = self.store.lock().unwrap();
        match &result {
            Ok(()) => {
                s.set("applied_revision", &revision)?;
                s.set("sync_error", "")?;
                *self.last_sync.lock().unwrap() = Some(std::time::Instant::now());
            }
            Err(e) => {
                *self.last_sync.lock().unwrap() = None;
                tracing::warn!(error=%e,"edge sync failed; retrying");
                s.set(
                    "sync_error",
                    "Edge synchronization failed; check the Caddy API and configuration directory. Retrying automatically.",
                )?;
            }
        }
        result
    }
}
pub fn router(app: App) -> Router {
    router_with_routes(app, Router::new())
}
pub fn router_with_routes(app: App, extra: Router<App>) -> Router {
    Router::new()
        .merge(extra)
        .merge(edition::routes(app.edition))
        .merge(tenant::routes())
        .route(
            "/",
            get(|State(a): State<App>| async move {
                let multi = a
                    .store
                    .lock()
                    .unwrap()
                    .db
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM users WHERE role='admin')",
                        [],
                        |r| r.get::<_, bool>(0),
                    )
                    .unwrap_or(true);
                Html(if multi {
                    include_str!("../../../web/portal.html")
                } else {
                    include_str!("../../../web/legacy/index.html")
                })
            }),
        )
        .route(
            "/legacy",
            get(|| async { Html(include_str!("../../../web/legacy/index.html")) }),
        )
        .route(
            "/portal.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../web/portal.js"),
                )
            }),
        )
        .route(
            "/portal.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../../../web/portal.css"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../web/legacy/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../../../web/style.css"),
                )
            }),
        )
        .route(
            "/i18n.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../web/i18n.js"),
                )
            }),
        )
        .route(
            "/locales/catalogs.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../../web/locales/catalogs.js"),
                )
            }),
        )
        .route(
            "/logo.png",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/png")],
                    include_bytes!("../../../web/logo.png").as_slice(),
                )
            }),
        )
        .route(
            "/favicon.ico",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "image/x-icon")],
                    include_bytes!("../../../web/favicon.ico").as_slice(),
                )
            }),
        )
        .route("/healthz", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/readyz", get(ready))
        .route("/api/v1/audit", get(audit))
        .route("/api/v1/diagnostics", get(diagnostics))
        .route("/api/v1/dashboard", get(dashboard))
        .route("/api/v1/sites", get(sites).post(create_site))
        .route(
            "/api/v1/sites/{id}",
            axum::routing::put(update_site).delete(delete_site),
        )
        .route("/api/v1/tunnels", get(tunnels).post(create_tunnel))
        .route(
            "/api/v1/tunnels/{id}",
            put(update_tunnel).delete(delete_tunnel),
        )
        .route("/api/v1/clients", get(clients).post(create_client))
        .route("/api/v1/clients/{id}/revoke", post(revoke_client))
        .route("/api/v1/settings", get(settings).put(update_settings))
        .route("/api/v1/settings/rotate-ask-key", post(rotate_ask))
        .route("/api/v1/sync", post(sync_now))
        .route("/api/v1/tls/check", get(tls_check))
        .route("/api/v1/client/config", get(client_config))
        .route("/api/v1/client/heartbeat", post(heartbeat))
        .layer(axum::extract::DefaultBodyLimit::max(16384))
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn_with_state(
            app.clone(),
            session_csrf,
        ))
        .with_state(app)
}
async fn session_csrf(
    State(a): State<App>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) && req.headers().contains_key(header::COOKIE)
        && !req.uri().path().starts_with("/api/v1/auth/")
        && req.uri().path() != "/api/v1/enroll"
        && !req.uri().path().starts_with("/api/v1/client/")
    {
        if let Err(e) = tenant::identity(&a, req.headers(), true) {
            return e.into_response();
        }
    }
    next.run(req).await
}
async fn security_headers(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("referrer-policy", "no-referrer".parse().unwrap());
    h.insert("cache-control", "no-store".parse().unwrap());
    h.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    res
}
async fn tunnels(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    Ok(Json(json!(a.store.lock().unwrap().tunnels()?)))
}
async fn clients(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    Ok(Json(json!(a.store.lock().unwrap().clients()?)))
}
#[derive(Deserialize)]
struct ClientInput {
    name: String,
}
async fn create_client(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<ClientInput>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    if i.name.trim().is_empty() || i.name.len() > 80 {
        return Err(bad("Name must be 1–80 bytes"));
    }
    let token = secret();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let s = a.store.lock().unwrap();
    tenant::legacy_write(&s)?;
    s.change("client.create", &id, |s| {
        s.db.execute(
            "INSERT INTO clients(id,name,token_hash) VALUES(?,?,?)",
            params![id, i.name.trim(), hash(&token)],
        )?;
        Ok(())
    })?;
    Ok(Json(json!({"id":id,"token":token})))
}
async fn revoke_client(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    tenant::legacy_write(&s)?;
    if !s.clients()?.iter().any(|c| c.id == id) {
        return Err(notfound());
    }
    s.change("client.revoke", &id, |s| {
        s.db.execute("UPDATE clients SET active=0,running=0 WHERE id=?", [&id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"pending"})))
}
fn save(
    a: &App,
    id: Option<String>,
    mut input: TunnelInput,
    tenant: Option<(tenant::User, Option<u64>)>,
) -> ApiResult<Json<Value>> {
    let s = a.store.lock().unwrap();
    if tenant.is_none() {
        tenant::legacy_write(&s)?;
    }
    let allocation = if let Some((u, bps)) = &tenant {
        Some(tenant::validate_tunnel(
            &s,
            u,
            id.as_deref(),
            &mut input,
            *bps,
            (a.port_start, a.port_end),
            &reserved_ports(a),
        )?)
    } else {
        None
    };
    input.validate().map_err(bad)?;
    if tenant.is_none()
        && s.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM client_owners WHERE client_id=?)",
            [&input.client_id],
            |r| r.get::<_, bool>(0),
        )?
    {
        return Err(bad(
            "Edit bandwidth-managed tunnels in the resource console",
        ));
    }
    if !s
        .clients()?
        .iter()
        .any(|c| c.id == input.client_id && c.active)
    {
        return Err(bad("Select a valid client"));
    }
    if input
        .domain
        .as_ref()
        .is_some_and(|d| Some(d) == a.panel_domain.as_ref())
    {
        return Err(bad("The management domain cannot be used for a tunnel"));
    }
    if input.domain.as_ref().is_some_and(|d| {
        s.sites()
            .map(|sites| sites.iter().any(|site| &site.domain == d))
            .unwrap_or(true)
    }) {
        return Err(Error(
            StatusCode::CONFLICT,
            "Domain is already used by a local website".into(),
        ));
    }
    let existing = s.tunnels()?;
    let previous = id
        .as_ref()
        .map(|id| existing.iter().find(|t| &t.id == id).ok_or_else(notfound))
        .transpose()?;
    let reserved = reserved_ports(a);
    let sites = s.sites()?;
    let used = |port: u16| {
        s.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM tunnel_limits WHERE backend_port=?)",
            [port],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(true)
            || reserved.contains(&port)
            || sites.iter().any(|s| s.upstream_port == port)
            || existing
                .iter()
                .any(|t| Some(&t.id) != id.as_ref() && t.public_port == port)
    };
    let port = input
        .public_port
        .filter(|p| *p != 0)
        .or_else(|| previous.map(|t| t.public_port))
        .or_else(|| (a.port_start..=a.port_end).find(|p| !used(*p)))
        .ok_or_else(|| bad("Port pool exhausted"))?;
    if port < a.port_start || port > a.port_end {
        return Err(bad(format!(
            "Public port must be in the range {}–{}",
            a.port_start, a.port_end
        )));
    }
    if used(port)
        || input.domain.as_ref().is_some_and(|d| {
            existing
                .iter()
                .any(|t| Some(&t.id) != id.as_ref() && t.domain.as_ref() == Some(d))
        })
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "Port or domain is already in use".into(),
        ));
    }
    let token = previous
        .filter(|t| t.client_id == input.client_id)
        .map(|t| t.service_token.clone())
        .unwrap_or_else(secret);
    let t = Tunnel {
        id: id.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()),
        name: input.name,
        client_id: input.client_id,
        protocol: input.protocol,
        local_host: input.local_host,
        local_port: input.local_port,
        public_port: port,
        domain: input.domain,
        enabled: input.enabled,
        service_token: token,
    };
    s.change(
        if previous.is_some() {
            "tunnel.update"
        } else {
            "tunnel.create"
        },
        &tenant.as_ref().map(|(u,_)|format!("{}:{}:{}",u.actor_id,u.id,t.id)).unwrap_or_else(||t.id.clone()),
        |s| {
            if let Some(old)=previous {if !edition::self_hosted(s) && old.domain!=t.domain {if let Some(d)=&old.domain{s.db.execute("INSERT INTO domain_cooldown VALUES(?,?) ON CONFLICT(domain) DO UPDATE SET expires=excluded.expires",params![d,now()+7*86400])?;}}}
            s.save_tunnel(&t)?;
            if let (Some((port,_)),Some((_,bps)))=(&allocation,&tenant){s.db.execute("INSERT INTO tunnel_limits VALUES(?,?,?) ON CONFLICT(tunnel_id) DO UPDATE SET bps=excluded.bps",params![t.id,bps.unwrap_or(0),port])?;
                s.db.execute("DELETE FROM tunnel_unlimited WHERE tunnel_id=?",[&t.id])?;
                if bps.is_none(){s.db.execute("INSERT INTO tunnel_unlimited VALUES(?)",[&t.id])?;}}
            Ok(())
        },
    )?;
    Ok(Json(json!(t)))
}
async fn create_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<TunnelInput>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    save(&a, None, i, None)
}
async fn update_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(i): Json<TunnelInput>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    save(&a, Some(id), i, None)
}
async fn delete_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    tenant::legacy_write(&s)?;
    if !s.tunnels()?.iter().any(|t| t.id == id) {
        return Err(notfound());
    }
    s.change("tunnel.delete", &id, |s| {
        s.db.execute("DELETE FROM tunnels WHERE id=?", [&id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"pending"})))
}
async fn settings(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    Ok(Json(
        json!({"config":s.settings()?,"port_start":a.port_start,"port_end":a.port_end,"ask_key_configured":true,"panel_domain":a.panel_domain}),
    ))
}
async fn update_settings(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<Settings>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    i.validate().map_err(bad)?;
    let s = a.store.lock().unwrap();
    s.change("settings.update", "local", |s| {
        s.set("config", &serde_json::to_string(&i)?)
    })?;
    Ok(Json(json!({"status":"pending"})))
}
async fn rotate_ask(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    s.change("tls.rotate_key", "local", |s| s.set("ask_key", &secret()))?;
    Ok(Json(json!({"status":"pending"})))
}
async fn sync_now(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    {
        let s = a.store.lock().unwrap();
        dirty(&s)?;
    }
    a.reconcile().await.map_err(|_| {
        Error(
            StatusCode::BAD_GATEWAY,
            "Configuration saved, but edge synchronization failed; retrying automatically".into(),
        )
    })?;
    Ok(Json(json!({"status":"applied"})))
}
#[derive(Deserialize)]
struct Ask {
    domain: Option<String>,
    key: Option<String>,
}
async fn tls_check(State(a): State<App>, Query(q): Query<Ask>) -> StatusCode {
    let s = a.store.lock().unwrap();
    let Ok(Some(key)) = s.get("ask_key") else {
        return StatusCode::FORBIDDEN;
    };
    if !bool::from(
        hash(q.key.as_deref().unwrap_or(""))
            .as_bytes()
            .ct_eq(hash(&key).as_bytes()),
    ) {
        return StatusCode::FORBIDDEN;
    }
    let Ok(d) = domain(q.domain.as_deref().unwrap_or("")) else {
        return StatusCode::FORBIDDEN;
    };
    if a.panel_domain.as_ref() == Some(&d) {
        return StatusCode::OK;
    }
    let allowed = s
        .active_tunnels()
        .map(|ts| ts.iter().any(|t| t.domain.as_deref() == Some(&d)))
        .unwrap_or(false);
    if allowed
        || s.sites()
            .map(|sites| sites.iter().any(|site| site.enabled && site.domain == d))
            .unwrap_or(false)
    {
        StatusCode::OK
    } else {
        StatusCode::FORBIDDEN
    }
}
async fn client_config(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let id = client(&a, &h)?;
    let s = a.store.lock().unwrap();
    let ts: Vec<_> = s
        .active_tunnels()?
        .into_iter()
        .filter(|t| t.client_id == id)
        .collect();
    let mut services = serde_json::Map::new();
    for t in &ts {
        services.insert(t.id.clone(),json!({"type":if t.protocol=="udp"{"udp"}else{"tcp"},"token":t.service_token,"local_addr":socket(&t.local_host,t.local_port)}));
    }
    use base64::Engine;
    let private = base64::engine::general_purpose::STANDARD
        .decode(s.get("noise_private")?.unwrap())
        .map_err(anyhow::Error::from)?;
    let bytes: [u8; 32] = private
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid noise key"))?;
    let public = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(bytes));
    let config = json!({"client":{"remote_addr":s.settings()?.public_addr,"transport":{"type":"noise","noise":{"remote_public_key":base64::engine::general_purpose::STANDARD.encode(public.as_bytes())}},"services":services}});
    Ok(Json(
        json!({"client_id":id,"tunnels":ts,"config":toml::to_string_pretty(&config).map_err(anyhow::Error::from)?,"revision":s.get("revision")?}),
    ))
}
#[derive(Deserialize)]
struct Heartbeat {
    running: bool,
}
async fn heartbeat(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<Heartbeat>,
) -> ApiResult<Json<Value>> {
    let id = client(&a, &h)?;
    a.store.lock().unwrap().db.execute(
        "UPDATE clients SET last_seen=?,running=? WHERE id=?",
        params![now(), i.running, id],
    )?;
    Ok(Json(json!({"status":"ok"})))
}
async fn dashboard(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    let clients = s.clients()?;
    let ts = s.active_tunnels()?;
    let m = a.metrics.lock().unwrap();
    let rev = s.get("revision")?;
    let applied = s.get("applied_revision")?;
    Ok(Json(
        json!({"cpu_percent":m.cpu,"memory_bytes":m.memory,"total_memory_bytes":m.total_memory,"center_memory_bytes":m.process_memory,"online_clients":clients.iter().filter(|c|c.active&&now()-c.last_seen<45).count(),"active_tunnels":ts.len(),"running_tunnels":ts.iter().filter(|t|clients.iter().any(|c|c.id==t.client_id&&c.running&&now()-c.last_seen<45)).count(),"traffic":m.samples,"traffic_scope":"host_network_interfaces","sync":{"status":if !s.get("sync_error")?.unwrap_or_default().is_empty(){"error"}else if rev==applied{"applied"}else{"pending"},"error":s.get("sync_error")?,"revision":rev,"applied_revision":applied}}),
    ))
}
pub fn start_background(a: App, logger: Arc<dyn TrafficLogger>) {
    let worker = a.clone();
    tokio::spawn(async move {
        loop {
            let _ = worker.reconcile().await;
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    });
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new();
        let mut networks = sysinfo::Networks::new_with_refreshed_list();
        let mut last = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            let pid = sysinfo::get_current_pid().unwrap();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
            networks.refresh(true);
            let elapsed = last.elapsed().as_secs_f64();
            last = std::time::Instant::now();
            let sample = TrafficSample {
                timestamp: now(),
                upload_bps: (networks.values().map(|n| n.transmitted()).sum::<u64>() as f64
                    / elapsed) as u64,
                download_bps: (networks.values().map(|n| n.received()).sum::<u64>() as f64
                    / elapsed) as u64,
            };
            {
                let mut m = a.metrics.lock().unwrap();
                m.cpu = sys.global_cpu_usage();
                m.memory = sys.used_memory();
                m.total_memory = sys.total_memory();
                m.process_memory = sys.process(pid).map(|p| p.memory()).unwrap_or(0);
                m.samples.push_back(sample.clone());
                while m.samples.len() > 60 {
                    m.samples.pop_front();
                }
            }
            if let Err(e) = logger.record("local", sample).await {
                tracing::warn!(error=%e,"traffic logger failed");
            }
        }
    });
}

async fn control_reachable(a: &App) -> bool {
    let Ok(mut address) = a.bind_addr.parse::<std::net::SocketAddr>() else {
        return false;
    };
    if address.ip().is_unspecified() {
        address.set_ip(if address.is_ipv4() {
            std::net::Ipv4Addr::LOCALHOST.into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::TcpStream::connect(address)
        )
        .await,
        Ok(Ok(_))
    )
}
fn synchronized(a: &App) -> bool {
    let fresh = a
        .last_sync
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed().as_secs() < 15);
    let s = a.store.lock().unwrap();
    fresh && s.get("revision").ok().flatten() == s.get("applied_revision").ok().flatten()
}
async fn ready(State(a): State<App>) -> StatusCode {
    if synchronized(&a) && control_reachable(&a).await {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
async fn audit(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    Ok(Json(json!(a.store.lock().unwrap().audit()?)))
}
async fn diagnostics(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let control = control_reachable(&a).await;
    let sync = synchronized(&a);
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"schema_version":4,"config_synchronized":sync,"rathole_control_listener_reachable":control,"ready":sync&&control,"note":"A reachable listener does not prove end-to-end health; check client logs and make a real application request"}),
    ))
}

fn reserved_ports(a: &App) -> Vec<u16> {
    let mut ports: Vec<u16> = [
        a.bind_addr.clone(),
        std::env::var("HP_BIND").unwrap_or("127.0.0.1:3000".into()),
        std::env::var("HP_CADDY_LISTEN").unwrap_or("127.0.0.1:2019".into()),
        std::env::var("HP_HTTPS_LISTEN").unwrap_or(":443".into()),
    ]
    .iter()
    .filter_map(|v| v.rsplit(':').next()?.parse().ok())
    .collect();
    ports.push(
        std::env::var("HP_HTTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(80),
    );
    ports
}
async fn sites(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    Ok(Json(json!(a.store.lock().unwrap().sites()?)))
}
fn save_site(a: &App, id: Option<String>, mut input: SiteInput) -> ApiResult<Json<Value>> {
    input.validate().map_err(bad)?;
    let s = a.store.lock().unwrap();
    let existing = s.sites()?;
    if id
        .as_ref()
        .is_some_and(|id| !existing.iter().any(|site| &site.id == id))
    {
        return Err(notfound());
    }
    let tunnels = s.tunnels()?;
    if a.panel_domain.as_ref() == Some(&input.domain)
        || existing
            .iter()
            .any(|site| Some(&site.id) != id.as_ref() && site.domain == input.domain)
        || tunnels
            .iter()
            .any(|t| t.domain.as_ref() == Some(&input.domain))
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "Domain is already used by a website, tunnel, or management panel".into(),
        ));
    }
    if s.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM tunnel_limits WHERE backend_port=?)",
        [input.upstream_port],
        |r| r.get::<_, bool>(0),
    )? || reserved_ports(a).contains(&input.upstream_port)
        || tunnels.iter().any(|t| t.public_port == input.upstream_port)
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "Application port conflicts with a management service, HTTPS endpoint, or tunnel listener".into(),
        ));
    }
    let action = if id.is_some() {
        "site.update"
    } else {
        "site.create"
    };
    let site = Site {
        id: id.unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string()),
        name: input.name,
        domain: input.domain,
        upstream_host: input.upstream_host,
        upstream_port: input.upstream_port,
        enabled: input.enabled,
    };
    s.change(action, &site.id, |s| s.save_site(&site))?;
    Ok(Json(json!(site)))
}
async fn create_site(
    State(a): State<App>,
    h: HeaderMap,
    Json(input): Json<SiteInput>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    save_site(&a, None, input)
}
async fn update_site(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<SiteInput>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    save_site(&a, Some(id), input)
}
async fn delete_site(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&a, &h)?;
    let s = a.store.lock().unwrap();
    if !s.sites()?.iter().any(|site| site.id == id) {
        return Err(notfound());
    }
    s.change("site.delete", &id, |s| {
        s.db.execute("DELETE FROM sites WHERE id=?", [&id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"pending"})))
}
