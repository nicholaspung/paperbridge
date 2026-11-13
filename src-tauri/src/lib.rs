//! Thin Tauri layer over pb-core. The webview never talks to the tablet;
//! it can only call the commands below, none of which accept a host, a URL
//! or a file path.

mod sync;

use pb_core::listing::DocumentRecord;
use pb_core::privacy::PrivacyStatus;
use pb_core::storage::Store;
use pb_core::ConnectionState;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tokio::sync::Notify;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Status {
    pub connection: ConnectionState,
    /// A sync with the tablet is running right now.
    pub syncing: bool,
    /// When the cached library was last refreshed, in ms since the Unix epoch.
    #[ts(type = "number | null")]
    pub last_synced_ms: Option<u64>,
    /// Name of the tablet's network interface, when one is present.
    pub interface: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub first_run_done: bool,
    /// "linux", "macos", …
    pub os: String,
}

pub struct AppState {
    pub store: Store,
    pub status: Mutex<Status>,
    pub refresh: Notify,
}

type Shared<'a> = State<'a, Arc<AppState>>;

fn store_err(e: impl std::fmt::Display) -> String {
    format!("local database error: {e}")
}

#[tauri::command]
fn get_status(state: Shared) -> Status {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
fn get_library(state: Shared) -> Result<Vec<DocumentRecord>, String> {
    state.store.load_library().map_err(store_err)
}

#[tauri::command]
fn refresh(state: Shared) {
    state.refresh.notify_one();
}

#[tauri::command]
fn get_privacy() -> PrivacyStatus {
    pb_core::privacy::check_host()
}

#[tauri::command]
fn get_settings(state: Shared) -> Result<Settings, String> {
    Ok(Settings {
        first_run_done: state.store.get("first_run_done").map_err(store_err)?.is_some(),
        os: std::env::consts::OS.to_string(),
    })
}

#[tauri::command]
fn complete_first_run(state: Shared) -> Result<(), String> {
    state.store.set("first_run_done", "1").map_err(store_err)
}

fn data_dir(app: &tauri::App) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    #[cfg(feature = "mock-device")]
    if let Some(dir) = std::env::var_os("PB_DATA_DIR") {
        return Ok(dir.into());
    }
    Ok(app.path().app_data_dir()?)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = data_dir(app)?;
            std::fs::create_dir_all(&dir)?;
            let store = Store::open(&dir.join("paperbridge.sqlite"))?;
            let last_synced_ms = store
                .get("last_synced_ms")
                .ok()
                .flatten()
                .and_then(|v| v.parse().ok());
            let state = Arc::new(AppState {
                store,
                status: Mutex::new(Status {
                    connection: ConnectionState::NoUsbInterface,
                    syncing: false,
                    last_synced_ms,
                    interface: None,
                }),
                refresh: Notify::new(),
            });
            app.manage(state.clone());
            tauri::async_runtime::spawn(sync::run(app.handle().clone(), state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_library,
            refresh,
            get_privacy,
            get_settings,
            complete_first_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running PaperBridge");
}
