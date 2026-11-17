//! "Open": export a document as an annotated PDF into the app's cache and
//! hand it to the system viewer. One job at a time; it shares the device
//! with the sync loop through `AppState::device`.

use crate::{sync, AppState, Shared};
use pb_core::device::{classify, Device, ExportFormat};
use pb_core::listing::{DocKind, DocumentRecord};
use pb_core::naming::safe_file_name;
use pb_core::transport::Timeouts;
use pb_core::{ConnectionState, DocId, Error};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

/// Exports larger than this are refused. Far above any real notebook.
const EXPORT_CAP: u64 = 4 << 30;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
#[ts(export)]
pub enum JobPhase {
    /// Waiting for a library refresh to finish.
    Waiting,
    /// Request sent; the tablet renders the whole PDF before it answers.
    Rendering,
    Receiving {
        #[ts(type = "number")]
        bytes: u64,
    },
    Opening,
    Done,
    Failed { message: String },
    Cancelled,
}

impl JobPhase {
    pub fn is_active(&self) -> bool {
        !matches!(self, JobPhase::Done | JobPhase::Failed { .. } | JobPhase::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Job {
    pub document_id: String,
    pub title: String,
    pub phase: JobPhase,
    /// When the job started, in ms since the Unix epoch.
    #[ts(type = "number")]
    pub started_ms: u64,
    /// The local copy was current, so the tablet was not asked.
    pub from_cache: bool,
}

#[derive(Default)]
pub struct Jobs {
    pub current: Option<Job>,
    abort: Option<tauri::async_runtime::JoinHandle<()>>,
}

/// What the cache remembers about an exported copy.
#[derive(Serialize, Deserialize)]
struct CacheEntry {
    modified: Option<String>,
    file: String,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn set_phase(app: &AppHandle, state: &AppState, phase: JobPhase) {
    let mut jobs = state.jobs.lock().unwrap();
    if let Some(job) = jobs.current.as_mut() {
        if job.phase != phase {
            job.phase = phase;
            let _ = app.emit("job-changed", Some(job.clone()));
        }
    }
}

/// A plain-language reason for a failed export.
fn describe(err: &Error, first_byte: Duration) -> String {
    match err {
        Error::Timeout(_, what) if what.contains("start an export") => format!(
            "The tablet did not start sending the PDF within {} minutes. Check that it is awake; \
             if it stays stuck, switch the USB web interface off and on.",
            first_byte.as_secs() / 60
        ),
        Error::Timeout(..) => "The transfer stopped part-way. Check the cable and try again.".into(),
        Error::BadExport(_) => "The tablet sent something that is not a PDF. Nothing was saved.".into(),
        Error::Io(e) => format!("Could not save the file on this computer: {e}"),
        other => match classify(other) {
            ConnectionState::WebInterfaceOff => {
                "The tablet's USB web interface is off or the tablet is asleep.".into()
            }
            ConnectionState::WebInterfaceHung => {
                "The tablet stopped answering. Switch the USB web interface off and on.".into()
            }
            ConnectionState::LocalNetworkPermissionDenied => {
                "macOS blocked the connection. Allow PaperBridge under Privacy & Security → Local Network.".into()
            }
            _ => format!("The export failed: {other}"),
        },
    }
}

fn cached_copy(dir: &Path, rec: &DocumentRecord) -> Option<PathBuf> {
    let entry: CacheEntry = serde_json::from_slice(&std::fs::read(dir.join("source.json")).ok()?).ok()?;
    if entry.modified.is_none() || entry.modified != rec.modified {
        return None;
    }
    let path = dir.join(&entry.file);
    let mut head = [0u8; 5];
    use std::io::Read;
    std::fs::File::open(&path).ok()?.read_exact(&mut head).ok()?;
    (&head == b"%PDF-").then_some(path)
}

/// Replaces whatever is cached for this document with the new export.
fn commit_cache(dir: &Path, rec: &DocumentRecord, file: &Path) -> std::io::Result<()> {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let n = entry.file_name().to_string_lossy().into_owned();
        if n != name && !n.starts_with(".pb-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let meta = serde_json::to_vec(&CacheEntry { modified: rec.modified.clone(), file: name }).unwrap();
    let tmp = dir.join(".source.json.tmp");
    std::fs::write(&tmp, meta)?;
    std::fs::rename(tmp, dir.join("source.json"))
}

fn open_with_system(app: &AppHandle, path: &Path) -> Result<(), String> {
    #[cfg(feature = "mock-device")]
    if std::env::var_os("PB_OPEN_DRY_RUN").is_some() {
        return Ok(());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("The PDF was saved but could not be opened: {e}"))
}

async fn run_job(app: AppHandle, state: Arc<AppState>, rec: DocumentRecord, dir: PathBuf) {
    let timeouts = Timeouts::default();
    let first_byte = timeouts.first_byte;
    let result: Result<PathBuf, String> = async {
        let _device = state.device.lock().await;
        set_phase(&app, &state, JobPhase::Rendering);
        let (target, _) = sync::find_target().ok_or("The tablet is no longer connected.")?;
        let device = Device::new(&target, timeouts).map_err(|e| describe(&e, first_byte))?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the cache folder: {e}"))?;
        let dest = dir.join(safe_file_name(&rec.title, "pdf"));
        // An older copy under the same name would block the no-replace rename.
        let _ = std::fs::remove_file(&dest);
        let mut last = (Instant::now(), 0u64);
        let (app2, state2) = (app.clone(), state.clone());
        let mut progress = move |bytes: u64| {
            // Throttle: at most ~5 updates a second.
            if bytes == 0 || last.0.elapsed() >= Duration::from_millis(200) {
                last = (Instant::now(), bytes);
                set_phase(&app2, &state2, JobPhase::Receiving { bytes });
            }
        };
        let out = device
            .export(&rec.id, ExportFormat::Pdf, &dest, EXPORT_CAP, &mut progress)
            .await
            .map_err(|e| {
                // Let the sync loop re-check the connection and update the badge.
                state.refresh.notify_one();
                describe(&e, first_byte)
            })?;
        commit_cache(&dir, &rec, &out.path).map_err(|e| format!("Could not update the cache: {e}"))?;
        Ok(out.path)
    }
    .await;
    let phase = match result {
        Ok(path) => {
            set_phase(&app, &state, JobPhase::Opening);
            match open_with_system(&app, &path) {
                Ok(()) => JobPhase::Done,
                Err(message) => JobPhase::Failed { message },
            }
        }
        Err(message) => JobPhase::Failed { message },
    };
    set_phase(&app, &state, phase);
    state.jobs.lock().unwrap().abort = None;
}

#[tauri::command]
pub fn open_document(app: AppHandle, state: Shared, id: String) -> Result<(), String> {
    let id = DocId::parse(&id).map_err(|e| e.to_string())?;
    let library = state.store.load_library().map_err(|e| e.to_string())?;
    let rec = library
        .into_iter()
        .find(|r| r.id == id)
        .ok_or("That document is no longer in the library.")?;
    if rec.kind == DocKind::Folder {
        return Err("Folders cannot be opened as a PDF.".into());
    }
    let mut jobs = state.jobs.lock().unwrap();
    if jobs.current.as_ref().is_some_and(|j| j.phase.is_active()) {
        return Err("Another document is still being prepared.".into());
    }
    let dir = state.cache_dir.join("open").join(id.as_str());
    let mut job = Job {
        document_id: id.as_str().to_string(),
        title: rec.title.clone(),
        phase: JobPhase::Opening,
        started_ms: now_ms(),
        from_cache: false,
    };

    // An unchanged document opens from the local copy, even offline.
    if let Some(path) = cached_copy(&dir, &rec) {
        job.from_cache = true;
        job.phase = match open_with_system(&app, &path) {
            Ok(()) => JobPhase::Done,
            Err(message) => JobPhase::Failed { message },
        };
        jobs.current = Some(job.clone());
        let _ = app.emit("job-changed", Some(job));
        return Ok(());
    }
    if state.status.lock().unwrap().connection != ConnectionState::Connected {
        return Err("Connect the tablet to open this document. It has not been opened on this computer since it last changed.".into());
    }
    job.phase = JobPhase::Waiting;
    jobs.current = Some(job.clone());
    let _ = app.emit("job-changed", Some(job));
    let handle = tauri::async_runtime::spawn(run_job(app.clone(), state.inner().clone(), rec, dir));
    jobs.abort = Some(handle);
    Ok(())
}

/// Stops the running job. The temp file is removed when the task drops it.
#[tauri::command]
pub fn cancel_job(app: AppHandle, state: Shared) {
    let mut jobs = state.jobs.lock().unwrap();
    if let Some(handle) = jobs.abort.take() {
        handle.abort();
    }
    if let Some(job) = jobs.current.as_mut().filter(|j| j.phase.is_active()) {
        job.phase = JobPhase::Cancelled;
        let _ = app.emit("job-changed", Some(job.clone()));
    }
}

#[tauri::command]
pub fn dismiss_job(app: AppHandle, state: Shared) {
    let mut jobs = state.jobs.lock().unwrap();
    if jobs.current.as_ref().is_some_and(|j| !j.phase.is_active()) {
        jobs.current = None;
        let _ = app.emit("job-changed", None::<Job>);
    }
}

#[tauri::command]
pub fn get_job(state: Shared) -> Option<Job> {
    state.jobs.lock().unwrap().current.clone()
}
