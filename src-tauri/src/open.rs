//! "Open": get a document's annotated PDF (or, for books, the original EPUB)
//! into the app's cache, then show it in the app or hand it to the system. One job at a time;
//! it shares the device with the sync loop through `AppState::device`.
//!
//! The cache is keyed by the tablet's modified time. Before trusting a cached
//! copy the job lists the document's folder (one small request), because the
//! library cache can be older than the last stroke on the tablet.

use crate::{sync, AppState, Shared};
use pb_core::device::{classify, Device, ExportFormat};
use pb_core::listing::{DocKind, DocumentRecord};
use pb_core::fsutil::TempFile;
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

/// What to show: the tablet's annotated PDF, or a book's original EPUB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ViewFormat {
    Pdf,
    Epub,
}

impl ViewFormat {
    fn ext(self) -> &'static str {
        match self {
            ViewFormat::Pdf => "pdf",
            ViewFormat::Epub => "epub",
        }
    }
    fn signature(self) -> &'static [u8] {
        match self {
            ViewFormat::Pdf => b"%PDF-",
            ViewFormat::Epub => b"PK\x03\x04",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "phase", rename_all = "camelCase")]
#[ts(export)]
pub enum JobPhase {
    /// Waiting for a library refresh to finish.
    Waiting,
    /// Asking the tablet whether the document changed since the cached copy.
    Checking,
    /// Request sent; the tablet renders the whole PDF before it answers.
    Rendering,
    Receiving {
        #[ts(type = "number")]
        bytes: u64,
    },
    /// Handing the file to the system viewer.
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
    /// The local copy was current, so the tablet did not render anything.
    pub from_cache: bool,
    pub format: ViewFormat,
    /// Open in the system viewer instead of the app's own.
    pub external: bool,
    /// Something the user should know about the result.
    pub note: Option<String>,
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

fn update_job(app: &AppHandle, state: &AppState, f: impl FnOnce(&mut Job)) {
    let mut jobs = state.jobs.lock().unwrap();
    if let Some(job) = jobs.current.as_mut() {
        let before = job.clone();
        f(job);
        if *job != before {
            let _ = app.emit("job-changed", Some(job.clone()));
        }
    }
}

fn set_phase(app: &AppHandle, state: &AppState, phase: JobPhase) {
    update_job(app, state, |j| j.phase = phase);
}

/// A plain-language reason for a failed export.
fn describe(err: &Error, first_byte: Duration) -> String {
    match err {
        Error::Timeout(_, what) if what.contains("start an export") => format!(
            "The tablet did not start sending the PDF within {} minutes. Check that it is awake; \
             if it stays stuck, switch the USB web interface off and on.",
            first_byte.as_secs() / 60
        ),
        Error::Timeout(_, what) if what.contains("receiving") => {
            "The transfer stopped part-way. Check the cable and try again.".into()
        }
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

fn read_entry(dir: &Path, format: ViewFormat) -> Option<(CacheEntry, PathBuf)> {
    let entry: CacheEntry = serde_json::from_slice(&std::fs::read(dir.join("source.json")).ok()?).ok()?;
    let path = dir.join(&entry.file);
    let sig = format.signature();
    let mut head = vec![0u8; sig.len()];
    use std::io::Read;
    std::fs::File::open(&path).ok()?.read_exact(&mut head).ok()?;
    (head == sig).then_some((entry, path))
}

/// The cached copy, if it is current. A book's original EPUB never changes
/// on the tablet (annotations live elsewhere), so any copy of it is current.
fn cached_copy(dir: &Path, format: ViewFormat, modified: &Option<String>) -> Option<PathBuf> {
    let (entry, path) = read_entry(dir, format)?;
    (format == ViewFormat::Epub || (modified.is_some() && entry.modified == *modified)).then_some(path)
}

/// Whatever copy is cached, however old.
fn any_cached_copy(dir: &Path, format: ViewFormat) -> Option<PathBuf> {
    read_entry(dir, format).map(|(_, p)| p)
}

/// Replaces whatever is cached for this document with the new export.
fn commit_cache(dir: &Path, modified: &Option<String>, file: &Path) -> std::io::Result<()> {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let n = entry.file_name().to_string_lossy().into_owned();
        if n != name && !n.starts_with(".pb-") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let meta = serde_json::to_vec(&CacheEntry { modified: modified.clone(), file: name }).unwrap();
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
        .map_err(|e| format!("The file was saved but could not be opened: {e}"))
}

fn cache_dir_for(state: &AppState, id: &DocId, format: ViewFormat) -> PathBuf {
    let doc = state.cache_dir.join("open").join(id.as_str());
    // v1.5 kept the PDF directly in the document's folder; clear that out.
    if let Ok(entries) = std::fs::read_dir(&doc) {
        for e in entries.flatten() {
            if e.file_type().is_ok_and(|t| t.is_file()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    doc.join(format.ext())
}

/// Ends the job with a file in hand: the app's viewer picks it up from
/// `Done`; the system viewer is launched here.
fn finish(app: &AppHandle, state: &AppState, path: &Path, external: bool) -> JobPhase {
    if !external {
        return JobPhase::Done;
    }
    set_phase(app, state, JobPhase::Opening);
    match open_with_system(app, path) {
        Ok(()) => JobPhase::Done,
        Err(message) => JobPhase::Failed { message },
    }
}

async fn fetch(
    app: &AppHandle,
    state: &Arc<AppState>,
    rec: &DocumentRecord,
    dir: &Path,
    format: ViewFormat,
) -> Result<PathBuf, String> {
    let timeouts = Timeouts::default();
    let first_byte = timeouts.first_byte;
    let _device = state.device.lock().await;
    set_phase(app, state, JobPhase::Checking);
    let (target, _) = sync::find_target().ok_or("The tablet is no longer connected.")?;
    let device = Device::new(&target, timeouts).map_err(|e| describe(&e, first_byte))?;

    // What does the tablet say about this document right now?
    let fresh = match device.list(rec.parent.as_ref()).await {
        Ok(items) => items.into_iter().find(|r| r.id == rec.id),
        Err(e) => {
            state.refresh.notify_one();
            if let Some(path) = any_cached_copy(dir, format) {
                update_job(app, state, |j| {
                    j.from_cache = true;
                    j.note = Some(
                        "The tablet did not answer, so this is the copy saved earlier and may miss recent changes."
                            .into(),
                    );
                });
                return Ok(path);
            }
            return Err(describe(&e, first_byte));
        }
    };
    // Moved or changed since the last sync: refresh the library afterwards.
    let modified = match &fresh {
        Some(f) if f.modified == rec.modified => rec.modified.clone(),
        Some(f) => {
            state.refresh.notify_one();
            f.modified.clone()
        }
        None => {
            state.refresh.notify_one();
            None
        }
    };
    if let Some(path) = cached_copy(dir, format, &modified) {
        update_job(app, state, |j| j.from_cache = true);
        return Ok(path);
    }

    set_phase(app, state, JobPhase::Rendering);
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not create the cache folder: {e}"))?;
    let dest = dir.join(safe_file_name(&rec.title, format.ext()));
    // An older copy under the same name would block the no-replace rename.
    let _ = std::fs::remove_file(&dest);
    let mut last = Instant::now();
    let (app2, state2) = (app.clone(), state.clone());
    let mut progress = move |bytes: u64| {
        // Throttle: at most ~5 updates a second.
        if bytes == 0 || last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            set_phase(&app2, &state2, JobPhase::Receiving { bytes });
        }
    };
    let failed = |e: Error| {
        // Let the sync loop re-check the connection and update the badge.
        state.refresh.notify_one();
        describe(&e, first_byte)
    };
    match format {
        ViewFormat::Pdf => {
            device
                .export(&rec.id, ExportFormat::Pdf, &dest, EXPORT_CAP, &mut progress)
                .await
                .map_err(failed)?;
        }
        ViewFormat::Epub => {
            // The native archive carries the original book file.
            let archive = TempFile::new_in(dir, "rmdoc");
            device
                .export(&rec.id, ExportFormat::Rmdoc, archive.path(), EXPORT_CAP, &mut progress)
                .await
                .map_err(failed)?;
            let (src, id, out) = (archive.path().to_path_buf(), rec.id.clone(), dest.clone());
            tauri::async_runtime::spawn_blocking(move || {
                pb_core::rmdoc::extract_original(&src, &id, "epub", &out, &Default::default())
            })
            .await
            .map_err(|e| format!("Could not unpack the book: {e}"))?
            .map_err(|e| match e {
                Error::BadArchive(_) | Error::BadExport(_) => {
                    "The tablet's copy of this book has no usable EPUB file. Try “With annotations” instead.".to_string()
                }
                other => describe(&other, first_byte),
            })?;
        }
    }
    commit_cache(dir, &modified, &dest).map_err(|e| format!("Could not update the cache: {e}"))?;
    Ok(dest)
}

async fn run_job(
    app: AppHandle,
    state: Arc<AppState>,
    rec: DocumentRecord,
    dir: PathBuf,
    format: ViewFormat,
    external: bool,
) {
    let phase = match fetch(&app, &state, &rec, &dir, format).await {
        Ok(path) => finish(&app, &state, &path, external),
        Err(message) => JobPhase::Failed { message },
    };
    set_phase(&app, &state, phase);
    state.jobs.lock().unwrap().abort = None;
}

/// Starts getting a document's PDF. Progress arrives as `job-changed`
/// events; with `external` false the UI shows it when the job is `Done`.
#[tauri::command]
pub fn open_document(
    app: AppHandle,
    state: Shared,
    id: String,
    format: ViewFormat,
    external: bool,
) -> Result<(), String> {
    let id = DocId::parse(&id).map_err(|e| e.to_string())?;
    let library = state.store.load_library().map_err(|e| e.to_string())?;
    let rec = library
        .into_iter()
        .find(|r| r.id == id)
        .ok_or("That document is no longer in the library.")?;
    if rec.kind == DocKind::Folder {
        return Err("Folders cannot be opened as a PDF.".into());
    }
    if format == ViewFormat::Epub && rec.kind != DocKind::Epub {
        return Err("Only books imported as EPUB have an EPUB version.".into());
    }
    let mut jobs = state.jobs.lock().unwrap();
    if jobs.current.as_ref().is_some_and(|j| j.phase.is_active()) {
        return Err("Another document is still being prepared.".into());
    }
    let dir = cache_dir_for(&state, &id, format);
    let mut job = Job {
        document_id: id.as_str().to_string(),
        title: rec.title.clone(),
        phase: JobPhase::Waiting,
        started_ms: now_ms(),
        from_cache: false,
        format,
        external,
        note: None,
    };

    // A book's original never changes, so its saved copy needs no tablet.
    if format == ViewFormat::Epub {
        if let Some(path) = any_cached_copy(&dir, format) {
            job.from_cache = true;
            jobs.current = Some(job);
            drop(jobs);
            let phase = finish(&app, &state, &path, external);
            set_phase(&app, &state, phase);
            return Ok(());
        }
    }

    if state.status.lock().unwrap().connection != ConnectionState::Connected {
        // Offline: the saved copy is the best there is.
        let Some(path) = any_cached_copy(&dir, format) else {
            return Err("Connect the tablet to open this document. It has not been opened on this computer before.".into());
        };
        job.from_cache = true;
        if cached_copy(&dir, format, &rec.modified).is_none() {
            job.note = Some("The tablet is not connected, and this saved copy is older than the library shows.".into());
        }
        jobs.current = Some(job);
        drop(jobs);
        let phase = finish(&app, &state, &path, external);
        set_phase(&app, &state, phase);
        return Ok(());
    }
    jobs.current = Some(job.clone());
    let _ = app.emit("job-changed", Some(job));
    let handle = tauri::async_runtime::spawn(run_job(app.clone(), state.inner().clone(), rec, dir, format, external));
    jobs.abort = Some(handle);
    Ok(())
}

/// The cached PDF or EPUB of a document, for the app's viewers. Takes an
/// ID, never a path, and only reads the cache.
#[tauri::command]
pub fn get_document_file(state: Shared, id: String, format: ViewFormat) -> Result<tauri::ipc::Response, String> {
    let id = DocId::parse(&id).map_err(|e| e.to_string())?;
    let path = any_cached_copy(&cache_dir_for(&state, &id, format), format)
        .ok_or("This document has not been opened yet.")?;
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read the saved file: {e}"))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Opens the cached file of a document in the system's default app.
#[tauri::command]
pub fn open_external(app: AppHandle, state: Shared, id: String, format: ViewFormat) -> Result<(), String> {
    let id = DocId::parse(&id).map_err(|e| e.to_string())?;
    let path = any_cached_copy(&cache_dir_for(&state, &id, format), format)
        .ok_or("This document has not been opened yet.")?;
    open_with_system(&app, &path)
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
