//! Atomic, non-clobbering file commits (plan §7.3).

use std::io;
use std::path::{Path, PathBuf};

/// Renames `from` to `to` only if `to` does not exist, atomically where the
/// filesystem supports it. Never falls back to hard links (exFAT has none).
/// Fails with `AlreadyExists` when `to` is taken.
pub fn rename_noclobber(from: &Path, to: &Path) -> io::Result<()> {
    match atomic_noreplace(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(e) if unsupported(&e) => {
            // Check-then-rename. Callers hold the app's single writer for the
            // destination folder, so nothing else can create `to` in between.
            if to.symlink_metadata().is_ok() {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists, "destination exists"));
            }
            std::fs::rename(from, to)
        }
        Err(e) => Err(e),
    }
}

fn unsupported(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::ENOTSUP))
        || e.kind() == io::ErrorKind::Unsupported
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cstr(p: &Path) -> io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(p.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

#[cfg(target_os = "linux")]
fn atomic_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let (f, t) = (cstr(from)?, cstr(to)?);
    // The raw syscall avoids depending on a glibc new enough to wrap it.
    let r = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            f.as_ptr(),
            libc::AT_FDCWD,
            t.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(target_os = "macos")]
fn atomic_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let (f, t) = (cstr(from)?, cstr(to)?);
    let r = unsafe { libc::renamex_np(f.as_ptr(), t.as_ptr(), libc::RENAME_EXCL) };
    if r == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn atomic_noreplace(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "no atomic no-replace rename"))
}

/// Makes a rename in `dir` durable. A no-op where directories cannot be
/// opened for syncing.
pub fn sync_dir(dir: &Path) {
    #[cfg(target_os = "linux")]
    if let Ok(f) = std::fs::File::open(dir) {
        let _ = f.sync_all();
    }
    #[cfg(not(target_os = "linux"))]
    let _ = dir;
}

/// Deletes a temp file when dropped, including when the task that owns it is
/// cancelled. Call `keep` once the file has been committed.
pub struct TempFile {
    path: Option<PathBuf>,
}

impl TempFile {
    /// A fresh hidden name in `dir`. Nothing is created yet.
    pub fn new_in(dir: &Path, ext: &str) -> TempFile {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let name = format!(".pb-{}-{nanos:x}-{n}.{ext}.part", std::process::id());
        TempFile { path: Some(dir.join(name)) }
    }

    pub fn path(&self) -> &Path {
        self.path.as_deref().expect("temp file already kept")
    }

    pub fn keep(mut self) {
        self.path = None;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noclobber() {
        let dir = std::env::temp_dir().join(format!("pb-fsutil-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::write(&a, b"one").unwrap();
        rename_noclobber(&a, &b).unwrap();
        assert!(!a.exists());
        std::fs::write(&a, b"two").unwrap();
        let err = rename_noclobber(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&b).unwrap(), b"one");
        assert_eq!(std::fs::read(&a).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn temp_file_removed_on_drop() {
        let dir = std::env::temp_dir();
        let t = TempFile::new_in(&dir, "pdf");
        let p = t.path().to_path_buf();
        std::fs::write(&p, b"x").unwrap();
        drop(t);
        assert!(!p.exists());
        let t = TempFile::new_in(&dir, "pdf");
        let p = t.path().to_path_buf();
        std::fs::write(&p, b"x").unwrap();
        t.keep();
        assert!(p.exists());
        std::fs::remove_file(p).unwrap();
    }
}
