//! Background loop: watch for the tablet, sync the library into the cache,
//! and tell the UI when something changed.

use crate::{AppState, Status};
use pb_core::device::{classify, Device};
use pb_core::platform::find_tablet_interfaces;
use pb_core::transport::{Target, Timeouts};
use pb_core::ConnectionState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_secs(3);

/// The test-only override. Compiled out unless `mock-device` is enabled, so
/// a release build has no code that reads this variable.
#[cfg(feature = "mock-device")]
fn mock_target() -> Option<(Target, String)> {
    let url = std::env::var("PB_DEVICE_BASE_URL").ok()?;
    let addr: std::net::SocketAddr = url.strip_prefix("http://")?.trim_end_matches('/').parse().ok()?;
    addr.ip().is_loopback().then(|| (Target::Mock(addr), "mock".to_string()))
}

#[cfg(not(feature = "mock-device"))]
fn mock_target() -> Option<(Target, String)> {
    None
}

/// The tablet, if a verified USB interface is present. On systems where the
/// USB check is not implemented yet, a subnet match is accepted.
fn find_target() -> Option<(Target, String)> {
    if let Some(mock) = mock_target() {
        return Some(mock);
    }
    let iface = find_tablet_interfaces()
        .unwrap_or_default()
        .into_iter()
        .find(|i| i.is_known_tablet() || !cfg!(target_os = "linux"))?;
    let name = iface.name.clone();
    Some((Target::Tablet(iface), name))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn update(app: &AppHandle, state: &AppState, f: impl FnOnce(&mut Status)) {
    let mut status = state.status.lock().unwrap();
    let before = status.clone();
    f(&mut status);
    if *status != before {
        let _ = app.emit("status-changed", status.clone());
    }
}

pub async fn run(app: AppHandle, state: Arc<AppState>) {
    let mut want_sync = true;
    loop {
        match find_target() {
            None => {
                update(&app, &state, |s| {
                    s.connection = ConnectionState::NoUsbInterface;
                    s.interface = None;
                    s.syncing = false;
                });
                want_sync = true; // sync again when the tablet comes back
            }
            Some((target, name)) => {
                let connected =
                    state.status.lock().unwrap().connection == ConnectionState::Connected;
                if want_sync || !connected {
                    update(&app, &state, |s| {
                        s.interface = Some(name);
                        s.syncing = true;
                        if s.connection == ConnectionState::NoUsbInterface {
                            s.connection = ConnectionState::UsbInterfaceUp;
                        }
                    });
                    let result = sync_once(&target, &state).await;
                    let ok = result.is_ok();
                    update(&app, &state, |s| {
                        s.syncing = false;
                        match result {
                            Ok(at) => {
                                s.connection = ConnectionState::Connected;
                                s.last_synced_ms = Some(at);
                            }
                            Err(c) => s.connection = c,
                        }
                    });
                    if ok {
                        let _ = app.emit("library-updated", ());
                        want_sync = false;
                    }
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(POLL) => {}
            _ = state.refresh.notified() => want_sync = true,
        }
    }
}

async fn sync_once(target: &Target, state: &AppState) -> Result<u64, ConnectionState> {
    let device = Device::new(target, Timeouts::default()).map_err(|e| classify(&e))?;
    let records = device.walk().await.map_err(|e| classify(&e))?;
    let at = now_ms();
    let saved = state
        .store
        .replace_library(&records)
        .and_then(|_| state.store.set("last_synced_ms", &at.to_string()));
    saved.map_err(|e| ConnectionState::TransportError {
        detail: format!("could not save the library: {e}"),
    })?;
    Ok(at)
}
