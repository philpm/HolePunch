use crate::{
    edition::Edition,
    node::{LocalNode, NoopTrafficLogger},
    start_background, App,
};
use std::{path::PathBuf, sync::Arc};
pub async fn run(
    edition: Edition,
    make_router: fn(App) -> axum::Router,
    name: &str,
) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let dir = PathBuf::from(std::env::var("HP_DATA_DIR").unwrap_or("data".into()));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [arg] if arg == "--version" => {
            println!(
                "{name} {} ({})",
                env!("CARGO_PKG_VERSION"),
                edition.as_str()
            );
            return Ok(());
        }
        [arg] if arg == "--help" => {
            println!("{name} [--version | backup DEST.db | restore SOURCE.db]\nConfigure service with HP_* environment variables. Restore requires stopped Center.");
            return Ok(());
        }
        [cmd, dest] if cmd == "backup" => {
            crate::maintenance::backup(&dir.join("holepunch.db"), std::path::Path::new(dest))?;
            println!("Consistent database backup created");
            return Ok(());
        }
        [cmd, source] if cmd == "restore" => {
            crate::maintenance::restore(&dir, std::path::Path::new(source))?;
            println!("Database restored; previous state retained as before-restore-*.db");
            return Ok(());
        }
        [] => {}
        _ => anyhow::bail!("Unknown arguments; use --help"),
    }
    let _directory_lock = crate::maintenance::lock_directory(&dir)?;
    let token = std::env::var("HP_ADMIN_TOKEN").map_err(|_| {
        anyhow::anyhow!("Set HP_ADMIN_TOKEN to a random secret of at least 32 characters")
    })?;
    let node = Arc::new(LocalNode {
        path: dir.join("server.toml"),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
    });
    let app = App::new_with_edition(dir.join("holepunch.db"), &token, node, edition)?;
    start_background(app.clone(), Arc::new(NoopTrafficLogger));
    let bind = std::env::var("HP_BIND").unwrap_or("127.0.0.1:3000".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind,"HolePunch Center listening");
    axum::serve(listener, make_router(app))
        .with_graceful_shutdown(async {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("SIGTERM handler");
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        })
        .await?;
    Ok(())
}
