use axum::{
    routing::{get, post},
    Json, Router,
};
use holepunch_client::{Binding, Client};
use serde_json::json;
#[tokio::test]
async fn validates_binding_and_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(dir.path().to_path_buf(), "missing-rathole".into()).unwrap();
    assert!(c
        .bind(Binding {
            api_url: "http://remote.example.com".into(),
            token: "a".repeat(64)
        })
        .await
        .is_err());
    assert!(c
        .bind(Binding {
            api_url: "https://user:pass@example.com".into(),
            token: "a".repeat(64)
        })
        .await
        .is_err());
    assert!(!c.status().bound);
    let app=Router::new().route("/api/v1/client/config",get(||async{Json(json!({"client_id":"one","tunnels":[],"config":"[client]\nremote_addr = '127.0.0.1:2333'\n","revision":"1"}))})).route("/api/v1/client/heartbeat",post(||async{Json(json!({"ok":true}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    c.bind(Binding {
        api_url: format!("http://{address}"),
        token: "a".repeat(64),
    })
    .await
    .unwrap();
    assert!(c.status().bound);
    assert!(dir.path().join("client.toml").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.path().join("client.toml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert!(c.start().await.is_err());
    assert!(!c.running());
    server.abort();
    assert!(c.sync().await.is_err());
    assert!(c.status().error.is_some());
    assert!(!c.running());
}
#[tokio::test]
async fn rejects_malformed_server_config_without_binding() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::new(dir.path().to_path_buf(), "rathole".into()).unwrap();
    let app=Router::new().route("/api/v1/client/config",get(||async{Json(json!({"client_id":"one","tunnels":[],"config":"invalid TOML ### [","revision":"1"}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    assert!(c
        .bind(Binding {
            api_url: format!("http://{address}"),
            token: "a".repeat(64)
        })
        .await
        .is_err());
    assert!(!dir.path().join("client.toml").exists());
    assert!(!c.status().bound);
    assert!(!c.running());
    server.abort();
}
