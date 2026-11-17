//! Thin Tauri layer over pb-core. The webview never talks to the tablet;
//! it can only call the commands below, none of which accept a host, a URL
//! or a file path.

mod open;
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
    /// Held by whoever is talking to the tablet (the sync loop or an
    /// export), so their requests never interleave.
    pub device: tokio::sync::Mutex<()>,
    pub jobs: Mutex<open::Jobs>,
    /// Exported copies for "Open". Safe to delete.
    pub cache_dir: std::path::PathBuf,
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

/// (data, cache) directories.
fn dirs(app: &tauri::App) -> Result<(std::path::PathBuf, std::path::PathBuf), Box<dyn std::error::Error>> {
    #[cfg(feature = "mock-device")]
    if let Some(dir) = std::env::var_os("PB_DATA_DIR") {
        let dir = std::path::PathBuf::from(dir);
        return Ok((dir.clone(), dir.join("cache")));
    }
    Ok((app.path().app_data_dir()?, app.path().app_cache_dir()?))
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let (dir, cache_dir) = dirs(app)?;
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
                device: tokio::sync::Mutex::new(()),
                jobs: Mutex::new(open::Jobs::default()),
                cache_dir,
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
            complete_first_run,
            open::open_document,
            open::cancel_job,
            open::dismiss_job,
            open::get_job,
            open::get_document_file,
            open::open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running PaperBridge");
}
