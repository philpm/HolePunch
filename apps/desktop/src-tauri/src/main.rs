#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use holepunch_client::{Binding, Client, Status};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_autostart::ManagerExt;
type PendingRestore = Arc<tokio::sync::Mutex<Option<Binding>>>;
type Shared = Arc<tokio::sync::Mutex<Client>>;
fn credentials() -> Result<keyring::Entry, String> {
    keyring::Entry::new("dev.holepunch.client", "binding").map_err(|e| e.to_string())
}
#[tauri::command]
async fn bind(
    api_url: String,
    token: String,
    remember: bool,
    state: tauri::State<'_, Shared>,
    pending: tauri::State<'_, PendingRestore>,
) -> Result<(), String> {
    let binding = Binding { api_url, token };
    state
        .lock()
        .await
        .bind(binding.clone())
        .await
        .map_err(|e| e.to_string())?;
    *pending.lock().await = None;
    if remember {
        credentials()?
            .set_password(&serde_json::to_string(&binding).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Bound, but saving system credentials failed: {e}"))?;
    } else {
        match credentials()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                return Err(format!(
                    "Bound, but removing previous credentials failed: {e}"
                ))
            }
        }
    }
    Ok(())
}
#[tauri::command]
async fn status(state: tauri::State<'_, Shared>) -> Result<Status, String> {
    Ok(state.lock().await.status())
}
#[tauri::command]
async fn start(state: tauri::State<'_, Shared>) -> Result<(), String> {
    state.lock().await.start().await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn stop(state: tauri::State<'_, Shared>) -> Result<(), String> {
    state.lock().await.stop().await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn sync(state: tauri::State<'_, Shared>) -> Result<(), String> {
    state.lock().await.sync().await.map_err(|e| e.to_string())
}
#[tauri::command]
async fn logs(state: tauri::State<'_, Shared>) -> Result<String, String> {
    state.lock().await.log_tail().map_err(|e| e.to_string())
}
#[tauri::command]
async fn forget(
    state: tauri::State<'_, Shared>,
    pending: tauri::State<'_, PendingRestore>,
) -> Result<(), String> {
    // Match worker lock ordering: client before pending restore.
    let mut client = state.lock().await;
    match credentials()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(e.to_string()),
    }
    *pending.lock().await = None;
    client.forget().await.map_err(|e| e.to_string())
}
#[tauri::command]
fn autostart(app: tauri::AppHandle, enabled: Option<bool>) -> Result<bool, String> {
    let manager = app.autolaunch();
    if let Some(enabled) = enabled {
        if enabled {
            manager.enable()
        } else {
            manager.disable()
        }
        .map_err(|e| e.to_string())?;
    }
    manager.is_enabled().map_err(|e| e.to_string())
}
struct TrayMenu {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}
#[tauri::command]
fn set_locale(locale: String, menu: tauri::State<'_, TrayMenu>) -> Result<(), String> {
    let source = match locale.as_str() {
        "en" => include_str!("../../../../web/locales/en.json"),
        "zh-CN" => include_str!("../../../../web/locales/zh-CN.json"),
        _ => return Err("Unsupported locale".into()),
    };
    let messages: serde_json::Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
    menu.show
        .set_text(
            messages["Show HolePunch"]
                .as_str()
                .unwrap_or("Show HolePunch"),
        )
        .map_err(|e| e.to_string())?;
    menu.quit
        .set_text(messages["Quit"].as_str().unwrap_or("Quit"))
        .map_err(|e| e.to_string())
}
fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let binary = std::env::var_os("HP_RATHOLE_BIN")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::current_exe()
                        .unwrap()
                        .with_file_name(if cfg!(windows) {
                            "holepunch-rathole.exe"
                        } else {
                            "holepunch-rathole"
                        })
                });
            let client: Shared = Arc::new(tokio::sync::Mutex::new(Client::new(data, binary)?));
            app.manage(client.clone());
            let restore = credentials()
                .and_then(|c| c.get_password().map_err(|e| e.to_string()))
                .ok()
                .and_then(|s| serde_json::from_str::<Binding>(&s).ok());
            let pending: PendingRestore = Arc::new(tokio::sync::Mutex::new(restore));
            app.manage(pending.clone());
            tauri::async_runtime::spawn(async move {
                loop {
                    let mut c = client.lock().await;
                    if c.status().bound {
                        *pending.lock().await = None;
                    }
                    let restore = pending.lock().await.clone();
                    if let Some(b) = restore {
                        match c.bind(b).await {
                            Ok(()) => {
                                *pending.lock().await = None;
                            }
                            Err(e) => {
                                c.error =
                                    Some(format!("Restoring the binding failed; retrying: {e}"));
                            }
                        }
                    } else if let Err(e) = c.tick().await {
                        c.error = Some(e.to_string());
                    }
                    drop(c);
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
            });
            let show = MenuItem::with_id(app, "show", "Show HolePunch", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            app.manage(TrayMenu { show, quit });
            let mut tray = TrayIconBuilder::new()
                .tooltip("HolePunch")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            bind, status, start, stop, sync, autostart, logs, forget, set_locale
        ])
        .build(tauri::generate_context!())
        .expect("failed to build HolePunch");
    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        if matches!(event, tauri::RunEvent::Exit) {
            tauri::async_runtime::block_on(async {
                let state = app.state::<Shared>();
                let _ = tokio::time::timeout(std::time::Duration::from_secs(12), async {
                    state.lock().await.stop().await
                })
                .await;
            });
        }
    });
}
