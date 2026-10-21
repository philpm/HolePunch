//! Shared identity, ownership and resource API. SaaS endpoints live in the separate Cloud repository.
use crate::*;
use argon2::{password_hash::phc::PasswordHash, Argon2, PasswordHasher, PasswordVerifier};
use rusqlite::OptionalExtension;
use serde::Serialize;

pub fn migrate(s: &Store) -> anyhow::Result<()> {
    s.db.execute_batch("BEGIN IMMEDIATE;
 CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY,email TEXT UNIQUE NOT NULL,password TEXT NOT NULL,role TEXT NOT NULL,status TEXT NOT NULL,verified INTEGER NOT NULL DEFAULT 0,created_at INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS user_limits(user_id TEXT PRIMARY KEY REFERENCES users(id),clients INTEGER,tunnels INTEGER,bps INTEGER);
 CREATE TABLE IF NOT EXISTS client_owners(client_id TEXT PRIMARY KEY REFERENCES clients(id),user_id TEXT NOT NULL REFERENCES users(id));
 CREATE TABLE IF NOT EXISTS tunnel_limits(tunnel_id TEXT PRIMARY KEY REFERENCES tunnels(id) ON DELETE CASCADE,bps INTEGER NOT NULL CHECK(bps>=0),backend_port INTEGER UNIQUE NOT NULL);
 CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY,user_id TEXT NOT NULL REFERENCES users(id),csrf TEXT NOT NULL,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS challenges(token_hash TEXT PRIMARY KEY,user_id TEXT NOT NULL REFERENCES users(id),purpose TEXT NOT NULL,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS enrollments(token_hash TEXT PRIMARY KEY,client_id TEXT NOT NULL REFERENCES clients(id),expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS mail_delivery(email TEXT NOT NULL,purpose TEXT NOT NULL,status TEXT NOT NULL,updated_at INTEGER NOT NULL,PRIMARY KEY(email,purpose));
 CREATE TABLE IF NOT EXISTS profiles(user_id TEXT PRIMARY KEY REFERENCES users(id),display_name TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS auth_attempts(key TEXT PRIMARY KEY,window INTEGER NOT NULL,count INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS domain_cooldown(domain TEXT PRIMARY KEY,expires INTEGER NOT NULL);
 CREATE TABLE IF NOT EXISTS tunnel_unlimited(tunnel_id TEXT PRIMARY KEY REFERENCES tunnels(id) ON DELETE CASCADE);
 PRAGMA user_version=4; COMMIT;")?;
    for (key, value) in [
        ("registration", "false"),
        ("default_clients", "1"),
        ("default_tunnels", "2"),
        ("default_bps", "1000000"),
        ("domain_suffix", ""),
    ] {
        if s.get(key)?.is_none() {
            s.set(key, value)?;
        }
    }
    Ok(())
}
fn password(raw: &str) -> ApiResult<String> {
    if !(12..=256).contains(&raw.len()) {
        return Err(bad("Password must be 12–256 bytes"));
    }
    Argon2::default()
        .hash_password(raw.as_bytes())
        .map(|p| p.to_string())
        .map_err(|_| bad("Password processing failed"))
}
async fn crypto_permit() -> ApiResult<tokio::sync::OwnedSemaphorePermit> {
    static POOL: std::sync::LazyLock<Arc<tokio::sync::Semaphore>> =
        std::sync::LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(4)));
    POOL.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Authentication is busy; try again later".into(),
        )
    })
}
pub async fn password_async(raw: String) -> ApiResult<String> {
    let permit = crypto_permit().await?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        password(&raw)
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))?
}
pub fn email(raw: &str) -> ApiResult<String> {
    let e = raw.trim().to_lowercase();
    if e.len() > 254
        || !e.contains('@')
        || e.contains(char::is_whitespace)
        || e.parse::<lettre::Address>().is_err()
    {
        return Err(bad("Enter a valid email address"));
    }
    Ok(e)
}
pub fn bootstrap(s: &Store) -> anyhow::Result<()> {
    if let (Ok(e), Ok(p)) = (
        std::env::var("HP_BOOTSTRAP_EMAIL"),
        std::env::var("HP_BOOTSTRAP_PASSWORD"),
    ) {
        if !e.is_empty() || !p.is_empty() {
            bootstrap_account(s, &e, &p)?;
        }
    }
    Ok(())
}
fn bootstrap_account(s: &Store, e: &str, p: &str) -> anyhow::Result<()> {
    let e = email(e).map_err(|_| anyhow::anyhow!("invalid bootstrap email"))?;
    let count: i64 =
        s.db.query_row("SELECT COUNT(*) FROM users WHERE role='admin'", [], |r| {
            r.get(0)
        })?;
    if count == 0 {
        let hashed = password(p)
            .map_err(|_| anyhow::anyhow!("bootstrap password needs at least 12 characters"))?;
        s.change("user.bootstrap", "bootstrap", |s| {
            s.db.execute(
                "INSERT INTO users VALUES('bootstrap',?,?,'admin','active',1,?)",
                params![e, hashed, now()],
            )?;
            s.db.execute(
                "INSERT INTO user_limits VALUES('bootstrap',10000,10000,1000000000)",
                [],
            )?;
            s.db.execute(
                "INSERT OR IGNORE INTO client_owners SELECT id,'bootstrap' FROM clients",
                [],
            )?;
            let tunnels = s.tunnels()?;
            let total = tunnels.len() as u64 * 1_000_000;
            s.db.execute(
                "UPDATE user_limits SET bps=MAX(bps,?) WHERE user_id='bootstrap'",
                [total],
            )?;
            for t in tunnels {
                let port = internal_port(s, (0, 0), &[])
                    .map_err(|_| anyhow::anyhow!("no internal port for legacy migration"))?;
                s.db.execute(
                    "INSERT OR IGNORE INTO tunnel_limits VALUES(?,?,?)",
                    params![
                        t.id,
                        if edition::self_hosted(s) {
                            0
                        } else {
                            1_000_000
                        },
                        port
                    ],
                )?;
                if edition::self_hosted(s) {
                    s.db.execute("INSERT OR IGNORE INTO tunnel_unlimited VALUES(?)", [&t.id])?;
                }
            }
            Ok(())
        })?;
    }

    Ok(())
}

#[derive(Clone, Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub verified: bool,
    #[serde(skip_serializing)]
    pub actor_id: String,
}
pub fn read_user(s: &Store, id: &str) -> ApiResult<User> {
    s.db.query_row(
        "SELECT id,email,role,status,verified FROM users WHERE id=?",
        [id],
        |r| {
            Ok(User {
                id: r.get(0)?,
                email: r.get(1)?,
                role: r.get(2)?,
                status: r.get(3)?,
                verified: r.get(4)?,
                actor_id: id.to_owned(),
            })
        },
    )
    .optional()?
    .ok_or_else(notfound)
}
fn cookie(h: &HeaderMap) -> Option<String> {
    h.get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|p| p.trim().strip_prefix("hp_session=").map(str::to_owned))
}
pub fn identity(a: &App, h: &HeaderMap, write: bool) -> ApiResult<User> {
    let token =
        cookie(h).ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Please sign in".into()))?;
    let s = a.store.lock().unwrap();
    let row: Option<(String, String)> =
        s.db.query_row(
            "SELECT user_id,csrf FROM sessions WHERE token_hash=? AND expires>?",
            params![hash(&token), now()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (id, csrf) =
        row.ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Session expired".into()))?;
    if write
        && !h
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| bool::from(v.as_bytes().ct_eq(csrf.as_bytes())))
    {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "CSRF validation failed; refresh the page".into(),
        ));
    }
    let u = read_user(&s, &id)?;
    if u.status != "active" {
        return Err(Error(StatusCode::FORBIDDEN, "Account is frozen".into()));
    }
    Ok(u)
}
pub fn manager(a: &App, h: &HeaderMap, write: bool) -> ApiResult<User> {
    let u = identity(a, h, write)?;
    if u.role != "admin" {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Administrator access required".into(),
        ));
    }
    Ok(u)
}
fn resource_identity(a: &App, h: &HeaderMap, write: bool) -> ApiResult<User> {
    let actor = identity(a, h, write)?;
    let Some(target) = h
        .get("x-manage-user")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
    else {
        return Ok(actor);
    };
    if a.edition == edition::Edition::SelfHosted {
        return Err(bad("Self-hosted does not support user delegation"));
    }
    if actor.role != "admin" {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Only administrators can manage other users".into(),
        ));
    }
    let mut user = read_user(&a.store.lock().unwrap(), target)?;
    user.actor_id = actor.id;
    Ok(user)
}
fn verified(u: &User) -> ApiResult<()> {
    if !u.verified {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Verify your email first".into(),
        ));
    }
    Ok(())
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Limits {
    pub clients: u64,
    pub tunnels: u64,
    pub bps: u64,
}
pub fn limits(s: &Store, id: &str) -> ApiResult<Limits> {
    let base = Limits {
        clients: s.get("default_clients")?.unwrap().parse().unwrap_or(1),
        tunnels: s.get("default_tunnels")?.unwrap().parse().unwrap_or(2),
        bps: s.get("default_bps")?.unwrap().parse().unwrap_or(1000000),
    };
    let v: Option<(Option<u64>, Option<u64>, Option<u64>)> =
        s.db.query_row(
            "SELECT clients,tunnels,bps FROM user_limits WHERE user_id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(if let Some((c, t, b)) = v {
        Limits {
            clients: c.unwrap_or(base.clients),
            tunnels: t.unwrap_or(base.tunnels),
            bps: b.unwrap_or(base.bps),
        }
    } else {
        base
    })
}
pub fn owns(s: &Store, u: &User, client_id: &str) -> ApiResult<()> {
    let owner: Option<String> =
        s.db.query_row(
            "SELECT user_id FROM client_owners WHERE client_id=?",
            [client_id],
            |r| r.get(0),
        )
        .optional()?;
    if owner.as_deref() != Some(&u.id) {
        return Err(notfound());
    }
    Ok(())
}
pub fn owner_active(s: &Store, id: &str) -> bool {
    s.db.query_row("SELECT NOT EXISTS(SELECT 1 FROM client_owners o JOIN users u ON u.id=o.user_id WHERE o.client_id=? AND (u.status!='active' OR u.verified=0))",[id],|r|r.get(0)).unwrap_or(false)
}
pub fn resource_allowed(s: &Store, t: &Tunnel) -> bool {
    let owner: Option<String> =
        s.db.query_row(
            "SELECT user_id FROM client_owners WHERE client_id=?",
            [&t.client_id],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let Some(owner) = owner else { return true };
    if edition::self_hosted(s) {
        return true;
    }
    let Ok(l) = limits(s, &owner) else {
        return false;
    };
    let Ok(cs) = s.clients() else { return false };
    let permitted=cs.iter().filter(|c|c.active&&s.db.query_row("SELECT EXISTS(SELECT 1 FROM client_owners WHERE client_id=? AND user_id=?)",params![c.id,owner],|r|r.get::<_,bool>(0)).unwrap_or(false)).take(l.clients as usize).any(|c|c.id==t.client_id);
    if !permitted {
        return false;
    }
    s.tunnels()
        .unwrap_or_default()
        .iter()
        .filter(|t| {
            s.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM client_owners WHERE client_id=? AND user_id=?)",
                params![t.client_id, owner],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
        })
        .take(l.tunnels as usize)
        .any(|v| v.id == t.id)
}
pub fn allocated(s: &Store, id: &str, except: &str) -> ApiResult<u64> {
    Ok(s.db.query_row("SELECT COALESCE(SUM(l.bps),0) FROM tunnel_limits l JOIN tunnels t ON t.id=l.tunnel_id JOIN client_owners o ON o.client_id=t.client_id WHERE o.user_id=? AND t.id<>?",params![id,except],|r|r.get(0))?)
}
pub fn count(s: &Store, id: &str, kind: &str) -> ApiResult<u64> {
    let sql = if kind == "clients" {
        "SELECT COUNT(*) FROM clients c JOIN client_owners o ON o.client_id=c.id WHERE o.user_id=? AND c.active=1"
    } else {
        "SELECT COUNT(*) FROM tunnels t JOIN client_owners o ON o.client_id=t.client_id WHERE o.user_id=?"
    };
    Ok(s.db.query_row(sql, [id], |r| r.get(0))?)
}
pub fn throttle(s: &Store, key: &str, max: i64) -> ApiResult<()> {
    s.db.execute("DELETE FROM auth_attempts WHERE window<?", [now() - 3600])?;
    s.db.execute("INSERT INTO auth_attempts VALUES(?,?,1) ON CONFLICT(key) DO UPDATE SET count=CASE WHEN window<? THEN 1 ELSE count+1 END,window=CASE WHEN window<? THEN excluded.window ELSE window END",params![hash(key),now(),now()-600,now()-600])?;
    let n: i64 = s.db.query_row(
        "SELECT count FROM auth_attempts WHERE key=?",
        [hash(key)],
        |r| r.get(0),
    )?;
    if n > max {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many requests; try again later".into(),
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}
async fn login(State(a): State<App>, Json(i): Json<Credentials>) -> ApiResult<Response> {
    let e = email(&i.email)?;
    let row = {
        let s = a.store.lock().unwrap();
        throttle(&s, "login-global", 120)?;
        throttle(&s, &format!("login:{e}"), 15)?;
        s.db.query_row("SELECT id,password FROM users WHERE email=?", [e], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .optional()?
    };
    static DUMMY: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| password("dummy-account-password").ok().unwrap());
    let known = row.is_some();
    let (id, p) = row.unwrap_or_else(|| (String::new(), DUMMY.clone()));
    let permit = crypto_permit().await?;
    let ok = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        PasswordHash::new(&p).ok().is_some_and(|h| {
            Argon2::default()
                .verify_password(i.password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))?;
    if !ok || !known {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Incorrect email or password".into(),
        ));
    }
    let token = secret();
    let csrf = secret();
    let s = a.store.lock().unwrap();
    let u = read_user(&s, &id)?;
    if u.status != "active" {
        return Err(Error(StatusCode::FORBIDDEN, "Account is frozen".into()));
    }
    s.db.execute("DELETE FROM sessions WHERE expires<?", [now()])?;
    s.db.execute(
        "INSERT INTO sessions VALUES(?,?,?,?)",
        params![hash(&token), id, csrf, now() + 86400],
    )?;
    let mut res = Json(json!({"user":u,"csrf":csrf})).into_response();
    let secure = if std::env::var("HP_ALLOW_HTTP").as_deref() == Ok("1") {
        ""
    } else {
        "; Secure"
    };
    res.headers_mut().insert(
        header::SET_COOKIE,
        format!("hp_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400{secure}")
            .parse()
            .unwrap(),
    );
    Ok(res)
}
async fn me(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    let l = limits(&s, &u.id)?;
    let b = allocated(&s, &u.id, "")?;
    let csrf: String = s.db.query_row(
        "SELECT csrf FROM sessions WHERE token_hash=?",
        [hash(&cookie(&h).unwrap())],
        |r| r.get(0),
    )?;
    let mut data = json!({"profile":profile(&s,&u.id)?,"user":u,"csrf":csrf,"limits":l,"allocated_bps":b,"remaining_bps":l.bps.saturating_sub(b),"client_count":count(&s,&u.id,"clients")?,"tunnel_count":count(&s,&u.id,"tunnels")?,"domain_suffix":s.get("domain_suffix")?,"public_addr":s.settings()?.public_addr,"capabilities":a.edition.capabilities()});
    if u.role == "admin" {
        data["setup_complete"] = json!(s.get("setup_complete")?.as_deref() == Some("true"));
    }
    if edition::self_hosted(&s) {
        data["limits"] = json!({"clients":null,"tunnels":null,"bps":null});
        data["allocated_bps"] = Value::Null;
        data["remaining_bps"] = Value::Null;
    }
    Ok(Json(data))
}
async fn logout(State(a): State<App>, h: HeaderMap) -> ApiResult<Response> {
    identity(&a, &h, true)?;
    a.store.lock().unwrap().db.execute(
        "DELETE FROM sessions WHERE token_hash=?",
        [hash(&cookie(&h).unwrap())],
    )?;
    let mut r = Json(json!({"status":"ok"})).into_response();
    r.headers_mut().insert(
        header::SET_COOKIE,
        "hp_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
            .parse()
            .unwrap(),
    );
    Ok(r)
}
async fn devices(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    Ok(Json(json!(s
        .clients()?
        .into_iter()
        .filter(|c| owns(&s, &u, &c.id).is_ok())
        .collect::<Vec<_>>())))
}
async fn create_device(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<ClientInput>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    verified(&u)?;
    if i.name.trim().is_empty() || i.name.len() > 80 {
        return Err(bad("Device name must be 1–80 bytes"));
    }
    let s = a.store.lock().unwrap();
    if !edition::self_hosted(&s) && count(&s, &u.id, "clients")? >= limits(&s, &u.id)?.clients {
        return Err(Error(
            StatusCode::CONFLICT,
            "Client count limit reached".into(),
        ));
    }
    let id = secret();
    let enroll = secret();
    s.change("client.create", &format!("{}:{id}", u.actor_id), |s| {
        s.db.execute(
            "INSERT INTO clients(id,name,token_hash) VALUES(?,?,?)",
            params![id, i.name.trim(), hash(&secret())],
        )?;
        s.db.execute("INSERT INTO client_owners VALUES(?,?)", params![id, u.id])?;
        s.db.execute(
            "INSERT INTO enrollments VALUES(?,?,?)",
            params![hash(&enroll), id, now() + 600],
        )?;
        Ok(())
    })?;
    Ok(Json(
        json!({"id":id,"enroll_token":enroll,"expires_in":600}),
    ))
}
async fn enroll_again(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    verified(&u)?;
    let s = a.store.lock().unwrap();
    owns(&s, &u, &id)?;
    if !s.clients()?.iter().any(|c| c.id == id && c.active) {
        return Err(notfound());
    }
    let code = secret();
    s.change("client.enrollment", &format!("{}:{id}", u.actor_id), |s| {
        s.db.execute("DELETE FROM enrollments WHERE client_id=?", [&id])?;
        s.db.execute(
            "INSERT INTO enrollments VALUES(?,?,?)",
            params![hash(&code), id, now() + 600],
        )?;
        Ok(())
    })?;
    Ok(Json(json!({"enroll_token":code,"expires_in":600})))
}
#[derive(Deserialize)]
struct EnrollInput {
    token: String,
}
async fn enroll(State(a): State<App>, Json(i): Json<EnrollInput>) -> ApiResult<Json<Value>> {
    let s = a.store.lock().unwrap();
    throttle(&s, "enroll-global", 120)?;
    let id:Option<String>=s.db.query_row("SELECT e.client_id FROM enrollments e JOIN clients c ON c.id=e.client_id JOIN client_owners o ON o.client_id=c.id JOIN users u ON u.id=o.user_id WHERE e.token_hash=? AND e.expires>? AND c.active=1 AND u.status='active' AND u.verified=1",params![hash(&i.token),now()],|r|r.get(0)).optional()?;
    let id = id.ok_or_else(|| bad("Enrollment code is invalid, already redeemed, or expired"))?;
    let token = secret();
    s.change("client.enroll", &id, |s| {
        s.db.execute("DELETE FROM enrollments WHERE client_id=?", [&id])?;
        s.db.execute(
            "UPDATE clients SET token_hash=? WHERE id=?",
            params![hash(&token), id],
        )?;
        // Rotate all data credentials too: replacing an old enrollment must evict old channels.
        for t in s.tunnels()?.into_iter().filter(|t| t.client_id == id) {
            s.db.execute(
                "UPDATE tunnels SET service_token=? WHERE id=?",
                params![secret(), t.id],
            )?;
        }
        Ok(())
    })?;
    Ok(Json(json!({"client_id":id,"token":token})))
}
async fn revoke_device(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    let s = a.store.lock().unwrap();
    owns(&s, &u, &id)?;
    s.change("client.revoke", &format!("{}:{id}", u.actor_id), |s| {
        for t in s.tunnels()?.iter().filter(|t| t.client_id == id) {
            cooldown(s, t)?;
        }
        s.db.execute("DELETE FROM tunnels WHERE client_id=?", [&id])?;
        s.db.execute("DELETE FROM enrollments WHERE client_id=?", [&id])?;
        s.db.execute("UPDATE clients SET active=0,running=0 WHERE id=?", [&id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"pending"})))
}
fn cooldown(s: &Store, t: &Tunnel) -> anyhow::Result<()> {
    if edition::self_hosted(s) {
        return Ok(());
    }
    if let Some(d) = &t.domain {
        s.db.execute("INSERT INTO domain_cooldown VALUES(?,?) ON CONFLICT(domain) DO UPDATE SET expires=excluded.expires",params![d,now()+7*86400])?;
    }
    Ok(())
}
async fn list_tunnels(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    let mut result = vec![];
    for t in s
        .tunnels()?
        .into_iter()
        .filter(|t| owns(&s, &u, &t.client_id).is_ok())
    {
        let mut v = json!(t);
        v["bandwidth_limit_bps"] = json!(s
            .db
            .query_row(
                "SELECT bps FROM tunnel_limits WHERE tunnel_id=?",
                [&t.id],
                |r| r.get::<_, u64>(0)
            )
            .optional()?
            .unwrap_or(0));
        let unlimited: bool = s.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM tunnel_unlimited WHERE tunnel_id=?)",
            [&t.id],
            |r| r.get(0),
        )?;
        if unlimited {
            v["bandwidth_limit_bps"] = Value::Null;
        }
        v["quota_paused"] = json!(!resource_allowed(&s, &t));
        v["policy_status"] = json!(if s.get("revision")? == s.get("applied_revision")? {
            "applied"
        } else {
            "pending"
        });
        result.push(v);
    }
    Ok(Json(json!(result)))
}
#[derive(Deserialize)]
struct TunnelRequest {
    #[serde(flatten)]
    input: TunnelInput,
    bandwidth_limit_bps: Option<u64>,
}
async fn new_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<TunnelRequest>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    verified(&u)?;
    save_tenant(&a, &u, None, i)
}
async fn edit_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(i): Json<TunnelRequest>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    verified(&u)?;
    save_tenant(&a, &u, Some(id), i)
}
fn save_tenant(
    a: &App,
    u: &User,
    id: Option<String>,
    mut request: TunnelRequest,
) -> ApiResult<Json<Value>> {
    // Reuse validated legacy allocator, but ownership and quota checks execute under
    // the very same store lock inside save(), never as a separate preflight request.
    request.input.public_port = None;
    save(
        a,
        id,
        request.input,
        Some((u.clone(), request.bandwidth_limit_bps)),
    )
}
fn internal_port(s: &Store, public_pool: (u16, u16), reserved: &[u16]) -> ApiResult<u16> {
    let tunnels = s.tunnels()?;
    let sites = s.sites()?;
    (30000..40000)
        .find(|p| {
            !(public_pool.0..=public_pool.1).contains(p)
                && !reserved.contains(p)
                && !tunnels.iter().any(|t| t.public_port == *p)
                && !sites.iter().any(|t| t.upstream_port == *p)
                && !s
                    .db
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM tunnel_limits WHERE backend_port=?)",
                        [p],
                        |r| r.get::<_, bool>(0),
                    )
                    .unwrap_or(true)
                && std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()
                && std::net::UdpSocket::bind(("127.0.0.1", *p)).is_ok()
        })
        .ok_or_else(|| bad("Internal port pool exhausted"))
}
pub fn legacy_write(s: &Store) -> ApiResult<()> {
    let multi: bool = s.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE role='admin')",
        [],
        |r| r.get(0),
    )?;
    if multi {
        return Err(Error(
            StatusCode::CONFLICT,
            "Use the resource console to manage devices and tunnels in multi-user mode".into(),
        ));
    }
    Ok(())
}
pub fn validate_tunnel(
    s: &Store,
    u: &User,
    id: Option<&str>,
    i: &mut TunnelInput,
    bps: Option<u64>,
    public_pool: (u16, u16),
    reserved: &[u16],
) -> ApiResult<(u16, String)> {
    owns(s, u, &i.client_id)?;
    let l = limits(s, &u.id)?;
    if let Some(id) = id {
        let t = s
            .tunnels()?
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(notfound)?;
        owns(s, u, &t.client_id)?;
    } else if !edition::self_hosted(s) && count(s, &u.id, "tunnels")? >= l.tunnels {
        return Err(Error(
            StatusCode::CONFLICT,
            "Tunnel count limit reached".into(),
        ));
    }
    if !edition::self_hosted(s) && bps.is_none() {
        return Err(bad("Cloud tunnels require a bandwidth limit"));
    }
    if let Some(bps) = bps {
        if bps > 1_000_000_000_000
            || (!edition::self_hosted(s)
                && allocated(s, &u.id, id.unwrap_or(""))?
                    .checked_add(bps)
                    .is_none_or(|n| n > l.bps))
        {
            return Err(Error(
                StatusCode::CONFLICT,
                "Total tunnel bandwidth exceeds the user limit".into(),
            ));
        }
        if bps == 0 {
            i.enabled = false;
        }
    }
    if i.protocol == "web" {
        let suffix = s.get("domain_suffix")?.unwrap_or_default();
        let retained = id
            .and_then(|id| s.tunnels().ok()?.into_iter().find(|t| t.id == id))
            .and_then(|t| t.domain);
        let unchanged = i
            .domain
            .as_deref()
            .and_then(|d| domain(d).ok())
            .is_some_and(|d| Some(&d) == retained.as_ref());
        if suffix.is_empty()
            && !unchanged
            && (!edition::self_hosted(s) || i.domain.as_ref().is_none_or(|d| d.trim().is_empty()))
        {
            return Err(bad(
                "The administrator has not configured a platform domain suffix",
            ));
        }
        let d = i
            .domain
            .clone()
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| format!("{}.{}", &secret()[..12], suffix));
        let d = domain(&d).map_err(bad)?;
        if !unchanged && !edition::self_hosted(s) {
            let prefix = d
                .strip_suffix(&format!(".{suffix}"))
                .ok_or_else(|| bad("Choose a platform domain; custom domains are not available"))?;
            if prefix.contains('.') || ["admin", "api", "www", "mail", "panel"].contains(&prefix) {
                return Err(bad("Domain prefix is unavailable"));
            }
        }
        let reserved: bool = s.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM domain_cooldown WHERE domain=? AND expires>?)",
            params![d, now()],
            |r| r.get(0),
        )?;
        if reserved && !edition::self_hosted(s) {
            return Err(Error(
                StatusCode::CONFLICT,
                "Domain is in its reuse cooldown period".into(),
            ));
        }
        i.domain = Some(d);
    }
    let existing: Option<u16> = if let Some(id) = id {
        s.db.query_row(
            "SELECT backend_port FROM tunnel_limits WHERE tunnel_id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };
    let port = if let Some(p) = existing {
        p
    } else {
        internal_port(s, public_pool, reserved)?
    };
    Ok((port, u.id.clone()))
}
async fn remove_tunnel(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    let s = a.store.lock().unwrap();
    let t = s
        .tunnels()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(notfound)?;
    owns(&s, &u, &t.client_id)?;
    s.change("tunnel.delete", &format!("{}:{id}", u.actor_id), |s| {
        cooldown(s, &t)?;
        s.db.execute("DELETE FROM tunnels WHERE id=?", [id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"pending"})))
}
#[derive(Deserialize)]
pub struct Allocation {
    pub tunnel_id: String,
    pub bps: Option<u64>,
}
#[derive(Deserialize)]
struct Reallocation {
    allocations: Vec<Allocation>,
}
pub fn allocation_total(s: &Store, u: &User, rows: &[Allocation]) -> ApiResult<u64> {
    let mut total = allocated(s, &u.id, "")?;
    let mut seen = std::collections::HashSet::new();
    if rows.len() > 10000 {
        return Err(bad("Too many reallocation entries"));
    }
    for row in rows {
        if !seen.insert(&row.tunnel_id)
            || row.bps.is_some_and(|b| b > 1_000_000_000_000)
            || (row.bps.is_none() && !edition::self_hosted(s))
        {
            return Err(bad("Duplicate tunnel or invalid bandwidth"));
        }
        let t = s
            .tunnels()?
            .into_iter()
            .find(|t| t.id == row.tunnel_id)
            .ok_or_else(notfound)?;
        owns(s, u, &t.client_id)?;
        let old: u64 = s.db.query_row(
            "SELECT bps FROM tunnel_limits WHERE tunnel_id=?",
            [&row.tunnel_id],
            |r| r.get(0),
        )?;
        total = total
            .checked_sub(old)
            .and_then(|n| n.checked_add(row.bps.unwrap_or(0)))
            .ok_or_else(|| bad("Bandwidth value overflow"))?;
    }
    Ok(total)
}
pub fn write_allocations(s: &Store, rows: &[Allocation]) -> anyhow::Result<()> {
    for row in rows {
        s.db.execute(
            "UPDATE tunnel_limits SET bps=? WHERE tunnel_id=?",
            params![row.bps.unwrap_or(0), row.tunnel_id],
        )?;
        s.db.execute(
            "DELETE FROM tunnel_unlimited WHERE tunnel_id=?",
            [&row.tunnel_id],
        )?;
        if row.bps.is_none() {
            s.db.execute("INSERT INTO tunnel_unlimited VALUES(?)", [&row.tunnel_id])?;
        }
        if row.bps == Some(0) {
            s.db.execute("UPDATE tunnels SET enabled=0 WHERE id=?", [&row.tunnel_id])?;
        }
    }
    Ok(())
}
async fn reallocate(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<Reallocation>,
) -> ApiResult<Json<Value>> {
    let u = resource_identity(&a, &h, true)?;
    verified(&u)?;
    let s = a.store.lock().unwrap();
    let total = allocation_total(&s, &u, &i.allocations)?;
    if !edition::self_hosted(&s) && total > limits(&s, &u.id)?.bps {
        return Err(Error(
            StatusCode::CONFLICT,
            "Total allocation exceeds the user limit".into(),
        ));
    }
    s.change(
        "bandwidth.reallocate",
        &format!("{}:{}", u.actor_id, u.id),
        |s| write_allocations(s, &i.allocations),
    )?;
    Ok(Json(json!({"status":"pending"})))
}

fn profile(s: &Store, id: &str) -> ApiResult<String> {
    Ok(s.db
        .query_row(
            "SELECT display_name FROM profiles WHERE user_id=?",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default())
}
#[derive(Deserialize)]
struct ProfileInput {
    display_name: String,
}
async fn update_profile(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<ProfileInput>,
) -> ApiResult<Json<Value>> {
    let u = identity(&a, &h, true)?;
    if i.display_name.len() > 80 {
        return Err(bad("Name must not exceed 80 bytes"));
    }
    let s = a.store.lock().unwrap();
    s.change("user.profile",&u.id,|s|{s.db.execute("INSERT INTO profiles VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET display_name=excluded.display_name",params![u.id,i.display_name.trim()])?;Ok(())})?;
    Ok(Json(json!({"status":"saved"})))
}
#[derive(Deserialize)]
struct PasswordInput {
    current_password: String,
    password: String,
}
pub(crate) async fn password_replacement(
    a: &App,
    u: &User,
    current_password: String,
    new_password: String,
) -> ApiResult<(String, String)> {
    let old: String = {
        let s = a.store.lock().unwrap();
        throttle(&s, &format!("password:{}", u.id), 10)?;
        s.db.query_row("SELECT password FROM users WHERE id=?", [&u.id], |r| {
            r.get(0)
        })?
    };
    let original = old.clone();
    let permit = crypto_permit().await?;
    let new = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if !PasswordHash::new(&old).ok().is_some_and(|p| {
            Argon2::default()
                .verify_password(current_password.as_bytes(), &p)
                .is_ok()
        }) {
            return Err(Error(
                StatusCode::UNAUTHORIZED,
                "Incorrect current password".into(),
            ));
        }
        password(&new_password)
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))??;
    Ok((original, new))
}
async fn change_password(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<PasswordInput>,
) -> ApiResult<Json<Value>> {
    let u = identity(&a, &h, true)?;
    let (original, new) = password_replacement(&a, &u, i.current_password, i.password).await?;
    let s = a.store.lock().unwrap();
    s.change("user.password", &u.id, |s| {
        anyhow::ensure!(
            s.db.execute(
                "UPDATE users SET password=? WHERE id=? AND password=?",
                params![new, u.id, original]
            )? == 1,
            "password changed concurrently"
        );
        s.db.execute("DELETE FROM sessions WHERE user_id=?", [&u.id])?;
        s.db.execute("DELETE FROM challenges WHERE user_id=?", [&u.id])?;
        Ok(())
    })?;
    Ok(Json(json!({"status":"login_required"})))
}
async fn sessions(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let u = identity(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    let current = hash(&cookie(&h).unwrap());
    let mut q =
        s.db.prepare("SELECT token_hash,expires FROM sessions WHERE user_id=? AND expires>?")?;
    let rows = q
        .query_map(params![u.id, now()], |r| {
            let id: String = r.get(0)?;
            Ok(json!({"id":id,"current":id==current,"expires":r.get::<_,i64>(1)?}))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(json!(rows)))
}
async fn remove_session(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let u = identity(&a, &h, true)?;
    let s = a.store.lock().unwrap();
    s.db.execute(
        "DELETE FROM sessions WHERE token_hash=? AND user_id=?",
        params![id, u.id],
    )?;
    Ok(Json(json!({"status":"revoked"})))
}
pub fn routes() -> Router<App> {
    Router::new()
        .route("/api/v1/me/profile", put(update_profile))
        .route("/api/v1/me/password", put(change_password))
        .route("/api/v1/me/sessions", get(sessions))
        .route(
            "/api/v1/me/sessions/{id}",
            axum::routing::delete(remove_session),
        )
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/me", get(me))
        .route("/api/v1/portal/bandwidth", post(reallocate))
        .route("/api/v1/portal/clients", get(devices).post(create_device))
        .route("/api/v1/portal/clients/{id}/enrollment", post(enroll_again))
        .route("/api/v1/portal/clients/{id}/revoke", post(revoke_device))
        .route("/api/v1/enroll", post(enroll))
        .route("/api/v1/portal/tunnels", get(list_tunnels).post(new_tunnel))
        .route(
            "/api/v1/portal/tunnels/{id}",
            put(edit_tunnel).delete(remove_tunnel),
        )
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn passwords_keep_phc_compatibility_and_random_salts() {
        // Argon2id vector also verified by argon2 0.5.3's PHC tests.
        let legacy = PasswordHash::new(
            "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ$CTFhFdXPJO1aFaMaO6Mm5c8y7cJHAph8ArZWb2GRPPc",
        )
        .unwrap();
        assert!(Argon2::default()
            .verify_password(b"password", &legacy)
            .is_ok());
        assert!(Argon2::default()
            .verify_password(b"incorrect", &legacy)
            .is_err());

        let raw = "local-compatibility-test-password";
        let first = password(raw).unwrap();
        let second = password(raw).unwrap();
        assert_ne!(first, second);
        for encoded in [first, second] {
            assert!(encoded.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
            let parsed = PasswordHash::new(&encoded).unwrap();
            assert!(Argon2::default()
                .verify_password(raw.as_bytes(), &parsed)
                .is_ok());
            assert!(Argon2::default()
                .verify_password(b"incorrect", &parsed)
                .is_err());
        }
    }

    #[test]
    fn bootstrap_preserves_legacy_credentials_and_adds_limits_once() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("test.db")).unwrap();
        s.db.execute(
            "INSERT INTO clients(id,name,token_hash) VALUES('legacy','old','same-hash')",
            [],
        )
        .unwrap();
        s.db.execute("INSERT INTO tunnels VALUES('old','old','legacy','tcp','127.0.0.1',8000,20000,NULL,1,'same-service-token')",[]).unwrap();
        bootstrap_account(&s, "admin@example.test", "long-local-test-password").unwrap();
        let before = s.get("revision").unwrap();
        bootstrap_account(&s, "ignored@example.test", "another-local-password").unwrap();
        assert_eq!(before, s.get("revision").unwrap());
        assert_eq!(allocated(&s, "bootstrap", "").unwrap(), 1_000_000);
        assert_eq!(s.tunnels().unwrap()[0].service_token, "same-service-token");
        assert_eq!(s.tunnels().unwrap()[0].public_port, 20000);
        assert_eq!(
            s.db.query_row(
                "SELECT token_hash FROM clients WHERE id='legacy'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "same-hash"
        );
        assert!(owns(&s, &read_user(&s, "bootstrap").unwrap(), "legacy").is_ok());
    }
}
