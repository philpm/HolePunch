//! Authenticated, atomic first-run server configuration shared by both editions.
use crate::*;

async fn status(State(a): State<App>, h: HeaderMap) -> ApiResult<Json<Value>> {
    let actor = tenant::manager(&a, &h, false)?;
    let s = a.store.lock().unwrap();
    let display_name: String = s.db.query_row(
        "SELECT COALESCE((SELECT display_name FROM profiles WHERE user_id=?),'')",
        [&actor.id],
        |row| row.get(0),
    )?;
    Ok(Json(json!({
        "complete": s.get("setup_complete")?.as_deref() == Some("true"),
        "email": actor.email, "display_name": display_name,
        "public_addr": s.settings()?.public_addr,
        "domain_suffix": s.get("domain_suffix")?.unwrap_or_default(),
        "panel_domain": a.panel_domain,
        "control_bind": a.bind_addr,
        "http_port": std::env::var("HP_HTTP_PORT").unwrap_or_else(|_| "80".into()),
        "https_listen": std::env::var("HP_HTTPS_LISTEN").unwrap_or_else(|_| ":443".into()),
        "port_start": a.port_start, "port_end": a.port_end
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupInput {
    display_name: String,
    public_addr: String,
    domain_suffix: String,
    #[serde(default)]
    current_password: String,
    #[serde(default)]
    password: String,
}

async fn complete(
    State(a): State<App>,
    h: HeaderMap,
    Json(input): Json<SetupInput>,
) -> ApiResult<Json<Value>> {
    let actor = tenant::manager(&a, &h, true)?;
    if input.display_name.len() > 80 {
        return Err(bad("Name must not exceed 80 bytes"));
    }
    let suffix = if input.domain_suffix.trim().is_empty() {
        String::new()
    } else {
        domain(&input.domain_suffix).map_err(bad)?
    };
    let public_addr = input.public_addr.trim().to_owned();
    {
        let mut config = a.store.lock().unwrap().settings()?;
        config.public_addr.clone_from(&public_addr);
        config.validate().map_err(bad)?;
    }
    let replacement = if input.password.is_empty() {
        if !input.current_password.is_empty() {
            return Err(bad(
                "Enter a new password or leave both password fields empty",
            ));
        }
        None
    } else {
        Some(
            tenant::password_replacement(&a, &actor, input.current_password, input.password)
                .await
                .map_err(|error| {
                    if error.0 == StatusCode::UNAUTHORIZED {
                        bad(error.1)
                    } else {
                        error
                    }
                })?,
        )
    };
    // A password hash may finish after a session was revoked in another request.
    tenant::manager(&a, &h, true)?;
    let s = a.store.lock().unwrap();
    let mut config = s.settings()?;
    config.public_addr = public_addr;
    s.change("setup.complete", &actor.id, |s| {
        if let Some((original, new)) = &replacement {
            anyhow::ensure!(s.db.execute(
                "UPDATE users SET password=? WHERE id=? AND password=?",
                params![new, actor.id, original],
            )? == 1, "password changed concurrently");
            s.db.execute("DELETE FROM sessions WHERE user_id=?", [&actor.id])?;
            s.db.execute("DELETE FROM challenges WHERE user_id=?", [&actor.id])?;
        }
        s.set("config", &serde_json::to_string(&config)?)?;
        s.set("domain_suffix", &suffix)?;
        s.db.execute("INSERT INTO profiles VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET display_name=excluded.display_name", params![actor.id, input.display_name.trim()])?;
        s.set("setup_complete", "true")?;
        Ok(())
    })?;
    Ok(Json(
        json!({"complete": true, "login_required": replacement.is_some(), "status": "pending"}),
    ))
}

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/api/v1/setup", get(status).put(complete))
}
