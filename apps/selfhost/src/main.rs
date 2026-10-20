#[tokio::main]
async fn main() -> anyhow::Result<()> {
    holepunch_server_core::service::run(
        holepunch_server_core::edition::Edition::SelfHosted,
        holepunch_server_core::router,
        "holepunch-server",
    )
    .await
}
