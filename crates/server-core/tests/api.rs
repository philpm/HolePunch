use async_trait::async_trait;
use axum::{body::Body, http::Request, Router};
use holepunch_server_core::{
    node::{caddy_config, server_toml, NodeConfig, NodeManager},
    router, App,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tower::ServiceExt;
struct MockNode {
    fail: AtomicBool,
}
#[async_trait]
impl NodeManager for MockNode {
    async fn sync_config(&self, _: &str, c: NodeConfig) -> anyhow::Result<()> {
        let _: toml::Value = toml::from_str(&server_toml(&c)?)?;
        caddy_config(&c)?;
        anyhow::ensure!(!self.fail.load(Ordering::SeqCst), "offline");
        Ok(())
    }
}
const ADMIN: &str = "test-admin-token-at-least-32-characters";
fn setup() -> (tempfile::TempDir, App, Router, Arc<MockNode>) {
    let dir = tempfile::tempdir().unwrap();
    let node = Arc::new(MockNode {
        fail: AtomicBool::new(false),
    });
    let app = App::new(dir.path().join("test.db"), ADMIN, node.clone()).unwrap();
    let router = router(app.clone());
    (dir, app, router, node)
}
async fn req(
    r: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (u16, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let res = r
        .clone()
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let code = res.status().as_u16();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (code, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
async fn add_client(r: &Router, name: &str) -> Value {
    let (code, v) = req(
        r,
        "POST",
        "/api/v1/clients",
        Some(ADMIN),
        json!({"name":name}),
    )
    .await;
    assert_eq!(code, 200);
    v
}
fn tunnel(client: &Value, protocol: &str) -> Value {
    json!({"name":"\u{5bb6}\u{5ead} NAS","client_id":client["id"],"protocol":protocol,"local_host":"127.0.0.1","local_port":8080,"domain":if protocol=="web"{Some("Nas.Example.COM")}else{None::<&str>}})
}
#[tokio::test]
async fn auth_and_client_isolation() {
    let (_d, _a, r, _) = setup();
    assert_eq!(
        req(&r, "GET", "/api/v1/tunnels", None, Value::Null).await.0,
        401
    );
    let one = add_client(&r, "One").await;
    let two = add_client(&r, "Two").await;
    assert_eq!(
        req(
            &r,
            "GET",
            "/api/v1/tunnels",
            one["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        401
    );
    let (_, t) = req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&one, "tcp"),
    )
    .await;
    assert!(t.get("service_token").is_none());
    let (_, cfg) = req(
        &r,
        "GET",
        "/api/v1/client/config",
        one["token"].as_str(),
        Value::Null,
    )
    .await;
    assert_eq!(cfg["tunnels"].as_array().unwrap().len(), 1);
    assert!(cfg["config"].as_str().unwrap().contains("noise"));
    let (_, cfg) = req(
        &r,
        "GET",
        "/api/v1/client/config",
        two["token"].as_str(),
        Value::Null,
    )
    .await;
    assert_eq!(cfg["tunnels"].as_array().unwrap().len(), 0);
    assert!(!cfg["config"]
        .as_str()
        .unwrap()
        .contains(t["id"].as_str().unwrap()));
    assert_eq!(
        req(&r, "GET", "/api/v1/client/config", Some(ADMIN), Value::Null)
            .await
            .0,
        401
    );
}
#[tokio::test]
async fn lifecycle_ports_and_validation() {
    let (_d, _a, r, _) = setup();
    let c = add_client(&r, "NAS").await;
    let mut input = tunnel(&c, "tcp");
    let (_, t) = req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), input.clone()).await;
    assert_eq!(t["public_port"], 20000);
    input["public_port"] = t["public_port"].clone();
    assert_eq!(
        req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), input.clone())
            .await
            .0,
        409
    );
    input["public_port"] = json!(80);
    assert_eq!(
        req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), input.clone())
            .await
            .0,
        400
    );
    input["public_port"] = Value::Null;
    input["local_host"] = json!("evil\"\n[server]");
    assert_eq!(
        req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), input)
            .await
            .0,
        400
    );
    let mut input = tunnel(&c, "udp");
    input["enabled"] = json!(false);
    let path = format!("/api/v1/tunnels/{}", t["id"].as_str().unwrap());
    assert_eq!(req(&r, "PUT", &path, Some(ADMIN), input).await.0, 200);
    let (_, cfg) = req(
        &r,
        "GET",
        "/api/v1/client/config",
        c["token"].as_str(),
        Value::Null,
    )
    .await;
    assert_eq!(cfg["tunnels"], json!([]));
    assert_eq!(
        req(&r, "DELETE", &path, Some(ADMIN), Value::Null).await.0,
        200
    );
    assert_eq!(
        req(&r, "DELETE", &path, Some(ADMIN), Value::Null).await.0,
        404
    );
}
#[tokio::test]
async fn tls_ask_and_revocation() {
    let (_d, a, r, _) = setup();
    let c = add_client(&r, "NAS").await;
    let (_, t) = req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&c, "web"),
    )
    .await;
    assert_eq!(t["domain"], "nas.example.com");
    let key = a.store.lock().unwrap().get("ask_key").unwrap().unwrap();
    let path = format!("/api/v1/tls/check?domain=NAS.EXAMPLE.COM&key={key}");
    assert_eq!(req(&r, "GET", &path, None, Value::Null).await.0, 200);
    assert_eq!(
        req(
            &r,
            "GET",
            "/api/v1/tls/check?domain=nas.example.com",
            None,
            Value::Null
        )
        .await
        .0,
        403
    );
    assert_eq!(
        req(
            &r,
            "GET",
            &format!("/api/v1/tls/check?domain=unknown.example.com&key={key}"),
            None,
            Value::Null
        )
        .await
        .0,
        403
    );
    assert_eq!(
        req(
            &r,
            "POST",
            "/api/v1/tunnels",
            Some(ADMIN),
            tunnel(&c, "web")
        )
        .await
        .0,
        409
    );
    assert_eq!(
        req(
            &r,
            "POST",
            &format!("/api/v1/clients/{}/revoke", c["id"].as_str().unwrap()),
            Some(ADMIN),
            Value::Null
        )
        .await
        .0,
        200
    );
    assert_eq!(req(&r, "GET", &path, None, Value::Null).await.0, 403);
    assert_eq!(
        req(
            &r,
            "GET",
            "/api/v1/client/config",
            c["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        401
    );
    assert!(a.store.lock().unwrap().active_tunnels().unwrap().is_empty());
}
#[tokio::test]
async fn reconciliation_failure_retry_and_persistence() {
    let (d, a, r, node) = setup();
    let c = add_client(&r, "NAS").await;
    req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&c, "tcp"),
    )
    .await;
    node.fail.store(true, Ordering::SeqCst);
    assert!(a.reconcile().await.is_err());
    let (_, dashboard) = req(&r, "GET", "/api/v1/dashboard", Some(ADMIN), Value::Null).await;
    assert_eq!(dashboard["sync"]["status"], "error");
    node.fail.store(false, Ordering::SeqCst);
    a.reconcile().await.unwrap();
    let (_, dashboard) = req(&r, "GET", "/api/v1/dashboard", Some(ADMIN), Value::Null).await;
    assert_eq!(dashboard["sync"]["status"], "applied");
    let reopened = App::new(d.path().join("test.db"), ADMIN, node).unwrap();
    assert_eq!(reopened.store.lock().unwrap().tunnels().unwrap().len(), 1);
    let db = std::fs::read(d.path().join("test.db")).unwrap();
    assert!(!db
        .windows(c["token"].as_str().unwrap().len())
        .any(|w| w == c["token"].as_str().unwrap().as_bytes()));
}
#[tokio::test]
async fn settings_and_heartbeat() {
    let (_d, _a, r, _) = setup();
    assert_eq!(req(&r,"PUT","/api/v1/settings",Some(ADMIN),json!({"caddy_url":"file:///etc/passwd","public_addr":"host:2333","tls_ask_url":"http://localhost/check"})).await.0,400);
    let c = add_client(&r, "Desktop").await;
    assert_eq!(
        req(
            &r,
            "POST",
            "/api/v1/client/heartbeat",
            c["token"].as_str(),
            json!({"running":true})
        )
        .await
        .0,
        200
    );
    let (_, d) = req(&r, "GET", "/api/v1/dashboard", Some(ADMIN), Value::Null).await;
    assert_eq!(d["online_clients"], 1);
    let (_, old) = req(&r, "GET", "/api/v1/settings", Some(ADMIN), Value::Null).await;
    assert!(old.get("ask_key").is_none());
}
#[tokio::test]
async fn concurrent_allocation() {
    let (_d, _a, r, _) = setup();
    let c = add_client(&r, "NAS").await;
    let mut jobs = Vec::new();
    for _ in 0..15 {
        let r = r.clone();
        let input = tunnel(&c, "tcp");
        jobs.push(tokio::spawn(async move {
            req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), input).await
        }));
    }
    let mut ports = std::collections::HashSet::new();
    for j in jobs {
        let (code, t) = j.await.unwrap();
        assert_eq!(code, 200);
        assert!(ports.insert(t["public_port"].as_u64().unwrap()));
    }
}
#[test]
fn domain_and_ipv6_validation() {
    use holepunch_server_core::model::*;
    for d in [
        "https://a.com",
        "*.a.com",
        "localhost",
        "127.0.0.1",
        "a..com",
        "-a.com",
        "a.com/path",
        "a.com:443",
    ] {
        assert!(domain(d).is_err(), "{d}");
    }
    assert_eq!(domain("A.Example.COM.").unwrap(), "a.example.com");
    assert_eq!(socket("::1", 80), "[::1]:80");
    let mut i: TunnelInput = serde_json::from_value(
        json!({"name":"IPv6","client_id":"x","protocol":"tcp","local_host":"::1","local_port":80}),
    )
    .unwrap();
    assert!(i.validate().is_ok());
}

#[tokio::test]
async fn tls_disabled_and_rotated_keys_fail_closed() {
    let (_d, a, r, _) = setup();
    let c = add_client(&r, "NAS").await;
    let (_, t) = req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&c, "web"),
    )
    .await;
    let old = a.store.lock().unwrap().get("ask_key").unwrap().unwrap();
    assert_eq!(
        req(
            &r,
            "POST",
            "/api/v1/settings/rotate-ask-key",
            Some(ADMIN),
            json!({})
        )
        .await
        .0,
        200
    );
    assert_eq!(
        req(
            &r,
            "GET",
            &format!("/api/v1/tls/check?domain=nas.example.com&key={old}"),
            None,
            Value::Null
        )
        .await
        .0,
        403
    );
    let new = a.store.lock().unwrap().get("ask_key").unwrap().unwrap();
    let path = format!("/api/v1/tls/check?domain=nas.example.com&key={new}");
    assert_eq!(req(&r, "GET", &path, None, Value::Null).await.0, 200);
    let mut input = tunnel(&c, "web");
    input["enabled"] = json!(false);
    req(
        &r,
        "PUT",
        &format!("/api/v1/tunnels/{}", t["id"].as_str().unwrap()),
        Some(ADMIN),
        input,
    )
    .await;
    assert_eq!(req(&r, "GET", &path, None, Value::Null).await.0, 403);
}

#[tokio::test]
async fn exhausted_port_pool_does_not_overwrite_tunnels() {
    let (_d, mut a, _, _) = setup();
    a.port_end = a.port_start;
    let r = router(a);
    let c = add_client(&r, "NAS").await;
    assert_eq!(
        req(
            &r,
            "POST",
            "/api/v1/tunnels",
            Some(ADMIN),
            tunnel(&c, "tcp")
        )
        .await
        .0,
        200
    );
    assert_eq!(
        req(
            &r,
            "POST",
            "/api/v1/tunnels",
            Some(ADMIN),
            tunnel(&c, "udp")
        )
        .await
        .0,
        400
    );
    assert_eq!(
        req(&r, "GET", "/api/v1/tunnels", Some(ADMIN), Value::Null)
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn audit_and_readiness_are_not_liveness() {
    let (_d, a, r, _) = setup();
    assert_eq!(req(&r, "GET", "/healthz", None, Value::Null).await.0, 200);
    assert_eq!(req(&r, "GET", "/readyz", None, Value::Null).await.0, 503);
    assert_eq!(
        req(&r, "GET", "/api/v1/audit", None, Value::Null).await.0,
        401
    );
    let client = add_client(&r, "audit client").await;
    req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&client, "tcp"),
    )
    .await;
    let (_, events) = req(&r, "GET", "/api/v1/audit", Some(ADMIN), Value::Null).await;
    assert_eq!(events.as_array().unwrap().len(), 2);
    assert!(!events
        .to_string()
        .contains(client["token"].as_str().unwrap()));
    let s = a.store.lock().unwrap();
    let rev = s.get("revision").unwrap();
    let result = s.change("fail", "test", |s| -> anyhow::Result<()> {
        s.set("should_rollback", "value")?;
        anyhow::bail!("injected failure")
    });
    assert!(result.is_err());
    assert_eq!(s.get("should_rollback").unwrap(), None);
    assert_eq!(s.get("revision").unwrap(), rev);
}
#[test]
fn future_schema_is_rejected_without_migration() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("db");
    let c = rusqlite::Connection::open(&p).unwrap();
    c.pragma_update(None, "user_version", 99).unwrap();
    assert!(holepunch_server_core::store::Store::open(&p).is_err());
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
}

#[tokio::test]
async fn same_host_sites_lifecycle_isolation_and_collisions() {
    let (d, mut a, _, node) = setup();
    a.panel_domain = Some("panel.example.com".into());
    let r = router(a.clone());
    let device = add_client(&r, "device").await;
    let input = json!({"name":" Blog ","domain":"Blog.Example.COM.","upstream_host":"localhost","upstream_port":20000,"enabled":true});
    assert_eq!(
        req(&r, "POST", "/api/v1/sites", None, input.clone())
            .await
            .0,
        401
    );
    assert_eq!(
        req(
            &r,
            "GET",
            "/api/v1/sites",
            device["token"].as_str(),
            Value::Null
        )
        .await
        .0,
        401
    );
    let (code, site) = req(&r, "POST", "/api/v1/sites", Some(ADMIN), input.clone()).await;
    assert_eq!(code, 200);
    assert_eq!(site["domain"], "blog.example.com");
    assert_eq!(site["upstream_host"], "127.0.0.1");
    assert_eq!(
        req(&r, "POST", "/api/v1/sites", Some(ADMIN), input.clone())
            .await
            .0,
        409
    );
    let path = format!("/api/v1/sites/{}", site["id"].as_str().unwrap());
    let ask_key = a.store.lock().unwrap().get("ask_key").unwrap().unwrap();
    let ask = format!("/api/v1/tls/check?domain=blog.example.com&key={ask_key}");
    assert_eq!(req(&r, "GET", &ask, None, Value::Null).await.0, 200);
    assert_eq!(
        req(
            &r,
            "GET",
            "/api/v1/tls/check?domain=blog.example.com",
            None,
            Value::Null
        )
        .await
        .0,
        403
    );
    for (field, value, expected) in [
        ("domain", json!("panel.example.com"), 409),
        ("upstream_host", json!("192.168.1.1"), 400),
        ("upstream_host", json!("example.com"), 400),
        ("upstream_port", json!(2019), 409),
        ("upstream_port", json!(3000), 409),
        ("upstream_port", json!(443), 409),
        ("upstream_port", json!(2333), 409),
        ("domain", json!("https://bad.example.com/path"), 400),
    ] {
        let mut bad = input.clone();
        bad[field] = value;
        assert_eq!(
            req(&r, "PUT", &path, Some(ADMIN), bad).await.0,
            expected,
            "{field}"
        );
    }
    // Allocator skips app ports, while domains are reserved in both directions.
    let (_, tcp) = req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&device, "tcp"),
    )
    .await;
    assert_eq!(tcp["public_port"], 20001);
    let mut web = tunnel(&device, "web");
    web["domain"] = json!("blog.example.com");
    assert_eq!(
        req(&r, "POST", "/api/v1/tunnels", Some(ADMIN), web).await.0,
        409
    );
    let (_, web) = req(
        &r,
        "POST",
        "/api/v1/tunnels",
        Some(ADMIN),
        tunnel(&device, "web"),
    )
    .await;
    let mut conflict = input.clone();
    conflict["domain"] = web["domain"].clone();
    assert_eq!(req(&r, "PUT", &path, Some(ADMIN), conflict).await.0, 409);
    let mut conflict = input.clone();
    conflict["upstream_port"] = tcp["public_port"].clone();
    assert_eq!(req(&r, "PUT", &path, Some(ADMIN), conflict).await.0, 409);
    a.reconcile().await.unwrap();
    {
        let s = a.store.lock().unwrap();
        let c = NodeConfig {
            backends: Default::default(),
            tunnels: s.active_tunnels().unwrap(),
            sites: s.sites().unwrap(),
            settings: s.settings().unwrap(),
            ask_key: ask_key.clone(),
            noise_private: "test".into(),
            bind_addr: "127.0.0.1:2333".into(),
            panel_domain: a.panel_domain.clone(),
        };
        let caddy = caddy_config(&c).unwrap().to_string();
        assert!(
            caddy.contains("blog.example.com")
                && caddy.contains("nas.example.com")
                && caddy.contains("panel.example.com")
        );
        assert!(!server_toml(&c)
            .unwrap()
            .contains(site["id"].as_str().unwrap()));
    }
    let config = req(
        &r,
        "GET",
        "/api/v1/client/config",
        device["token"].as_str(),
        Value::Null,
    )
    .await
    .1
    .to_string();
    assert!(!config.contains("blog.example.com"));
    req(
        &r,
        "POST",
        &format!("/api/v1/clients/{}/revoke", device["id"].as_str().unwrap()),
        Some(ADMIN),
        json!({}),
    )
    .await;
    assert_eq!(req(&r, "GET", &ask, None, Value::Null).await.0, 200);
    // Failed sync does not lose a site edit; retry includes it.
    node.fail.store(true, Ordering::SeqCst);
    let mut edited = input.clone();
    edited["enabled"] = json!(false);
    edited["upstream_host"] = json!("::1");
    assert_eq!(req(&r, "PUT", &path, Some(ADMIN), edited).await.0, 200);
    assert!(a.reconcile().await.is_err());
    assert_eq!(req(&r, "GET", &ask, None, Value::Null).await.0, 403);
    node.fail.store(false, Ordering::SeqCst);
    a.reconcile().await.unwrap();
    let restored = App::new(d.path().join("test.db"), ADMIN, node).unwrap();
    assert_eq!(
        restored.store.lock().unwrap().sites().unwrap()[0].upstream_host,
        "::1"
    );
    let audit = req(&r, "GET", "/api/v1/audit", Some(ADMIN), Value::Null)
        .await
        .1;
    assert!(audit.to_string().contains("site.update"));
    assert_eq!(
        req(&r, "DELETE", &path, Some(ADMIN), Value::Null).await.0,
        200
    );
    assert_eq!(req(&r, "GET", &ask, None, Value::Null).await.0, 403);
    assert_eq!(req(&r, "PUT", &path, Some(ADMIN), input).await.0, 404);
}

#[test]
fn sites_migration_and_backup_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    let s = holepunch_server_core::store::Store::open(&path).unwrap();
    s.set("preserve", "legacy").unwrap();
    s.db.execute_batch("DROP TABLE sites; PRAGMA user_version=1;")
        .unwrap();
    drop(s);
    let s = holepunch_server_core::store::Store::open(&path).unwrap();
    assert_eq!(s.get("preserve").unwrap().as_deref(), Some("legacy"));
    assert_eq!(
        s.db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    s.save_site(&holepunch_server_core::model::Site {
        id: "a".into(),
        name: "Blog".into(),
        domain: "blog.example.com".into(),
        upstream_host: "127.0.0.1".into(),
        upstream_port: 8080,
        enabled: true,
    })
    .unwrap();
    let backup = dir.path().join("backup.db");
    holepunch_server_core::maintenance::backup(&path, &backup).unwrap();
    let dest = dir.path().join("restored");
    holepunch_server_core::maintenance::restore(&dest, &backup).unwrap();
    let s = holepunch_server_core::store::Store::open(&dest.join("holepunch.db")).unwrap();
    assert_eq!(s.sites().unwrap()[0].domain, "blog.example.com");
}
