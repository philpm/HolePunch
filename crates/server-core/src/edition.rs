//! Product policy shared by both launchers. No account provisioning or SMTP here.
use crate::{store::Store, *};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    SelfHosted,
    Cloud,
}
impl Edition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfHosted => "self_hosted",
            Self::Cloud => "cloud",
        }
    }
    pub fn capabilities(self) -> Value {
        json!({"edition":self,"multi_user":self==Self::Cloud,"quotas":self==Self::Cloud,"email_auth":self==Self::Cloud,"custom_domains":self==Self::SelfHosted,"unlimited_bandwidth":self==Self::SelfHosted,"nodes":"local"})
    }
}
pub fn self_hosted(s: &Store) -> bool {
    s.get("edition").ok().flatten().as_deref() == Some("self_hosted")
}
pub fn configure(s: &Store, edition: Edition) -> anyhow::Result<()> {
    if let Some(saved) = s.get("edition")? {
        anyhow::ensure!(saved == edition.as_str(), "Data directory belongs to {saved}; use its matching server binary. Edition changes require an explicit migration.");
    }
    if edition == Edition::SelfHosted {
        let (users, admins): (i64, i64) = s.db.query_row(
            "SELECT COUNT(*),COALESCE(SUM(role='admin' AND status='active'),0) FROM users",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        anyhow::ensure!(
            users == 0 || (users == 1 && admins == 1),
            "Self-hosted requires a single administrator workspace; refusing a multi-user database"
        );
    }
    if s.get("setup_complete")?.is_none() {
        // Preserve existing deployments; only fresh databases enter setup automatically.
        s.set(
            "setup_complete",
            if s.get("config")?.is_some() {
                "true"
            } else {
                "false"
            },
        )?;
    }
    s.set("edition", edition.as_str())?;
    Ok(())
}
async fn capabilities(State(a): State<App>) -> Json<Value> {
    Json(a.edition.capabilities())
}
#[derive(Deserialize)]
struct DomainSettings {
    domain_suffix: String,
}
async fn domains(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    tenant::manager(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    Ok(Json(
        json!({"domain_suffix":s.get("domain_suffix")?.unwrap_or_default()}),
    ))
}
async fn update_domains(
    State(a): State<App>,
    h: HeaderMap,
    Json(i): Json<DomainSettings>,
) -> ApiResult<Json<Value>> {
    let actor = tenant::manager(&a, &h, true)?;
    let suffix = if i.domain_suffix.trim().is_empty() {
        String::new()
    } else {
        domain(&i.domain_suffix).map_err(bad)?
    };
    a.store
        .lock()
        .unwrap()
        .change("domains.update", &actor.id, |s| {
            s.set("domain_suffix", &suffix)
        })?;
    Ok(Json(json!({"status":"saved"})))
}
pub fn routes(edition: Edition) -> Router<App> {
    let router = Router::new()
        .route("/api/v1/capabilities", get(capabilities))
        .merge(crate::setup::routes());
    if edition == Edition::SelfHosted {
        router.route("/api/v1/domains", get(domains).put(update_domains))
    } else {
        router
    }
}
