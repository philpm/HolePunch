use async_trait::async_trait;
use axum::{body::Body, http::Request, Router};
use holepunch_server_core::{
    edition::Edition,
    model::{hash, now, Settings},
    node::{NodeConfig, NodeManager},
    router,
    store::Store,
    tenant::password_async,
    App,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

struct NoopNode;
#[async_trait]
impl NodeManager for NoopNode {
    async fn sync_config(&self, _: &str, _: NodeConfig) -> anyhow::Result<()> {
        Ok(())
    }
}
const OLD: &str = "initial-test-password";
const TOKEN: &str = "test-session";
const CSRF: &str = "test-csrf";
async fn fixture(existing: bool, edition: Edition) -> (tempfile::TempDir, App, Router) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let s = Store::open(&path).unwrap();
    let password = password_async(OLD.into()).await.unwrap();
    s.db.execute(
        "INSERT INTO users VALUES('owner','owner@example.test',?,'admin','active',1,?)",
        rusqlite::params![password, now()],
    )
    .unwrap();
    s.db.execute(
        "INSERT INTO sessions VALUES(?,'owner',?,?)",
        rusqlite::params![hash(TOKEN), CSRF, now() + 3600],
    )
    .unwrap();
    if existing {
        s.set(
            "config",
            &serde_json::to_string(&Settings {
                public_addr: "old.example.test:2333".into(),
                caddy_url: "http://127.0.0.1:2019".into(),
                tls_ask_url: "http://127.0.0.1:3000/api/v1/tls/check".into(),
            })
            .unwrap(),
        )
        .unwrap();
    }
    drop(s);
    let app = App::new_with_edition(
        path,
        "test-admin-token-at-least-32-bytes",
        Arc::new(NoopNode),
        edition,
    )
    .unwrap();
    let route = router(app.clone());
    (dir, app, route)
}
async fn request(
    r: &Router,
    method: &str,
    path: &str,
    session: bool,
    csrf: bool,
    input: Value,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if session {
        request = request.header("cookie", format!("hp_session={TOKEN}"));
    }
    if csrf {
        request = request.header("x-csrf-token", CSRF);
    }
    let response = r
        .clone()
        .oneshot(request.body(Body::from(input.to_string())).unwrap())
        .await
        .unwrap();
    let code = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (code, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
fn input() -> Value {
    json!({"display_name":"My server", "public_addr":"edge.example.test:12333", "domain_suffix":"t.example.test"})
}

#[tokio::test]
async fn setup_requires_admin_session_and_csrf_and_keeps_secrets_private() {
    let (_dir, app, r) = fixture(false, Edition::SelfHosted).await;
    assert_eq!(
        request(&r, "GET", "/api/v1/setup", false, false, Value::Null)
            .await
            .0,
        401
    );
    assert_eq!(
        request(&r, "PUT", "/api/v1/setup", true, false, input())
            .await
            .0,
        403
    );
    let (code, data) = request(&r, "GET", "/api/v1/setup", true, false, Value::Null).await;
    assert_eq!(code, 200);
    assert_eq!(data["complete"], false);
    for key in [
        "password",
        "current_password",
        "admin_token",
        "ask_key",
        "noise_private",
    ] {
        assert!(data.get(key).is_none());
    }
    app.store
        .lock()
        .unwrap()
        .db
        .execute("UPDATE users SET role='user' WHERE id='owner'", [])
        .unwrap();
    assert_eq!(
        request(&r, "PUT", "/api/v1/setup", true, true, input())
            .await
            .0,
        403
    );
}

#[tokio::test]
async fn setup_validation_does_not_partially_save_and_completion_persists() {
    let (dir, app, r) = fixture(false, Edition::SelfHosted).await;
    let before = app.store.lock().unwrap().settings().unwrap().public_addr;
    for (field, invalid) in [
        ("public_addr", "https://bad.example"),
        ("domain_suffix", "bad/domain"),
    ] {
        let mut body = input();
        body[field] = json!(invalid);
        assert_eq!(
            request(&r, "PUT", "/api/v1/setup", true, true, body)
                .await
                .0,
            400
        );
        let store = app.store.lock().unwrap();
        assert_eq!(store.settings().unwrap().public_addr, before);
        assert_eq!(
            store.get("setup_complete").unwrap().as_deref(),
            Some("false")
        );
    }
    assert_eq!(
        request(&r, "PUT", "/api/v1/setup", true, true, input())
            .await
            .0,
        200
    );
    let restored = App::new_with_edition(
        dir.path().join("test.db"),
        "test-admin-token-at-least-32-bytes",
        Arc::new(NoopNode),
        Edition::SelfHosted,
    )
    .unwrap();
    let s = restored.store.lock().unwrap();
    assert_eq!(s.get("setup_complete").unwrap().as_deref(), Some("true"));
    assert_eq!(s.settings().unwrap().public_addr, "edge.example.test:12333");
    assert_eq!(
        s.get("domain_suffix").unwrap().as_deref(),
        Some("t.example.test")
    );
    assert_eq!(
        s.db.query_row(
            "SELECT display_name FROM profiles WHERE user_id='owner'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "My server"
    );
}

#[tokio::test]
async fn setup_password_change_is_atomic_and_revokes_sessions() {
    let (_dir, app, r) = fixture(false, Edition::SelfHosted).await;
    let mut body = input();
    body["current_password"] = json!("wrong");
    body["password"] = json!("new-test-password-long");
    assert_eq!(
        request(&r, "PUT", "/api/v1/setup", true, true, body.clone())
            .await
            .0,
        400
    );
    assert_eq!(
        app.store
            .lock()
            .unwrap()
            .get("setup_complete")
            .unwrap()
            .as_deref(),
        Some("false")
    );
    body["current_password"] = json!(OLD);
    let (code, result) = request(&r, "PUT", "/api/v1/setup", true, true, body).await;
    assert_eq!(code, 200);
    assert_eq!(result["login_required"], true);
    assert_eq!(
        request(&r, "GET", "/api/v1/setup", true, false, Value::Null)
            .await
            .0,
        401
    );
    assert_eq!(
        request(
            &r,
            "POST",
            "/api/v1/auth/login",
            false,
            false,
            json!({"email":"owner@example.test","password":"new-test-password-long"})
        )
        .await
        .0,
        200
    );
    let audit = app.store.lock().unwrap().audit().unwrap();
    assert!(!serde_json::to_string(&audit)
        .unwrap()
        .contains("password-long"));
}

#[tokio::test]
async fn existing_installations_keep_settings_and_cloud_admin_has_setup() {
    let (_dir, app, r) = fixture(true, Edition::SelfHosted).await;
    let (_, status) = request(&r, "GET", "/api/v1/setup", true, false, Value::Null).await;
    assert_eq!(status["complete"], true);
    assert_eq!(
        app.store.lock().unwrap().settings().unwrap().public_addr,
        "old.example.test:2333"
    );
    let (_dir, app, r) = fixture(false, Edition::Cloud).await;
    assert_eq!(
        request(&r, "GET", "/api/v1/setup", true, false, Value::Null)
            .await
            .0,
        200
    );
    let (_, me) = request(&r, "GET", "/api/v1/me", true, false, Value::Null).await;
    assert_eq!(me["setup_complete"], false);
    assert_eq!(
        request(&r, "PUT", "/api/v1/setup", true, true, input())
            .await
            .0,
        200
    );
    app.store
        .lock()
        .unwrap()
        .db
        .execute("UPDATE users SET role='user' WHERE id='owner'", [])
        .unwrap();
    assert_eq!(
        request(&r, "GET", "/api/v1/setup", true, false, Value::Null)
            .await
            .0,
        403
    );
    let (_, me) = request(&r, "GET", "/api/v1/me", true, false, Value::Null).await;
    assert!(me.get("setup_complete").is_none());
}
