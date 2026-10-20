use async_trait::async_trait;
use axum::{body::Body, http::Request, Router};
use holepunch_server_core::{
    edition::Edition,
    model::{hash, now},
    node::{NodeConfig, NodeManager},
    router,
    store::Store,
    App,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
struct Node;
#[async_trait]
impl NodeManager for Node {
    async fn sync_config(&self, _: &str, _: NodeConfig) -> anyhow::Result<()> {
        Ok(())
    }
}
fn setup() -> (tempfile::TempDir, App, Router) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("test.db");
    {
        let s = Store::open(&path).unwrap();
        s.db.execute(
            "INSERT INTO users VALUES('owner','owner@example.test','unused','admin','active',1,0)",
            [],
        )
        .unwrap();
        s.db.execute(
            "INSERT INTO sessions VALUES(?,'owner','csrf',?)",
            rusqlite::params![hash("owner"), now() + 3600],
        )
        .unwrap();
    }
    let a = App::new_with_edition(
        path,
        "test-admin-secret-at-least-32-characters",
        Arc::new(Node),
        Edition::SelfHosted,
    )
    .unwrap();
    let r = router(a.clone());
    (d, a, r)
}
async fn req(r: &Router, path: &str, method: &str, data: Value) -> (u16, Value) {
    let response = r
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header("content-type", "application/json")
                .header("cookie", "hp_session=owner")
                .header("x-csrf-token", "csrf")
                .body(Body::from(data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let code = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (code, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}
#[tokio::test]
async fn selfhost_unlimited_custom_domains_and_zero_are_distinct() {
    let (_d, a, r) = setup();
    let (_, me) = req(&r, "/me", "GET", Value::Null).await;
    assert!(me["limits"]["bps"].is_null());
    assert_eq!(me["capabilities"]["multi_user"], false);
    let mut device = Value::Null;
    for i in 0..3 {
        let (code, row) = req(
            &r,
            "/portal/clients",
            "POST",
            json!({"name":format!("device-{i}")}),
        )
        .await;
        assert_eq!(code, 200, "{row}");
        device = row;
    }
    let new = |bps: Value, domain: &str| json!({"client_id":device["id"],"name":"web","protocol":"web","domain":domain,"local_host":"127.0.0.1","local_port":8000,"enabled":true,"bandwidth_limit_bps":bps});
    let mut ids = vec![];
    for i in 0..3 {
        let (code, row) = req(
            &r,
            "/portal/tunnels",
            "POST",
            new(Value::Null, &format!("custom{i}.example.org")),
        )
        .await;
        assert_eq!(code, 200, "{row}");
        assert_eq!(row["enabled"], true);
        ids.push(row["id"].as_str().unwrap().to_string());
    }
    let (_, rows) = req(&r, "/portal/tunnels", "GET", Value::Null).await;
    assert!(rows
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["bandwidth_limit_bps"].is_null() && t["quota_paused"] == false));
    let (code,_)=req(&r,"/portal/bandwidth","POST",json!({"allocations":[{"tunnel_id":ids[0],"bps":0},{"tunnel_id":ids[1],"bps":null},{"tunnel_id":ids[2],"bps":2000000000u64}]})).await;
    assert_eq!(code, 200);
    let (_, rows) = req(&r, "/portal/tunnels", "GET", Value::Null).await;
    let rows = rows.as_array().unwrap();
    assert_eq!(
        rows.iter().find(|r| r["id"] == ids[0]).unwrap()["enabled"],
        false
    );
    assert!(rows.iter().find(|r| r["id"] == ids[1]).unwrap()["bandwidth_limit_bps"].is_null());
    assert_eq!(
        req(
            &r,
            &format!("/portal/tunnels/{}", ids[1]),
            "DELETE",
            Value::Null
        )
        .await
        .0,
        200
    );
    assert_eq!(
        req(
            &r,
            "/portal/tunnels",
            "POST",
            new(Value::Null, "custom1.example.org")
        )
        .await
        .0,
        200,
        "selfhost domains have no tenant cooldown"
    );
    assert!(App::new_with_edition(
        a.store.lock().unwrap().db.path().unwrap().into(),
        "test-admin-secret-at-least-32-characters",
        Arc::new(Node),
        Edition::Cloud
    )
    .is_err());
}
#[tokio::test]
async fn selfhost_has_no_saas_routes_and_cannot_delegate() {
    let (_d, _a, r) = setup();
    for (path, method) in [
        ("/admin/users", "GET"),
        ("/admin/policy", "GET"),
        ("/auth/register", "POST"),
        ("/auth/request-reset", "POST"),
        ("/auth/verify", "POST"),
    ] {
        assert_eq!(req(&r, path, method, json!({})).await.0, 404, "{path}");
    }
    assert_eq!(
        req(
            &r,
            "/domains",
            "PUT",
            json!({"domain_suffix":"t.example.test"})
        )
        .await
        .0,
        200
    );
    let (code, domains) = req(&r, "/domains", "GET", Value::Null).await;
    assert_eq!(code, 200);
    assert_eq!(domains["domain_suffix"], "t.example.test");
    let response = r
        .oneshot(
            Request::builder()
                .uri("/api/v1/portal/clients")
                .header("cookie", "hp_session=owner")
                .header("x-manage-user", "owner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
}
#[test]
fn selfhost_refuses_an_unlabelled_multiuser_database() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("test.db");
    {
        let s = Store::open(&path).unwrap();
        for id in ["one", "two"] {
            s.db.execute(
                "INSERT INTO users VALUES(?1,?1,'unused','admin','active',1,0)",
                [id],
            )
            .unwrap();
        }
    }
    assert!(App::new_with_edition(
        path,
        "test-admin-secret-at-least-32-characters",
        Arc::new(Node),
        Edition::SelfHosted
    )
    .is_err());
}
