//! The only code that talks to the tablet (plan §4.1).
//!
//! - Fixed destination `http://10.11.99.1`; callers supply a path only.
//! - Socket bound to the tablet's USB interface, so traffic cannot leave by
//!   another route.
//! - No proxies, no redirects, one request in flight at a time.

use crate::error::{Error, Result};
use crate::platform::{TabletInterface, TABLET_IP};
pub use reqwest::Method;
use reqwest::{redirect, Client, Response};
use std::net::IpAddr;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone)]
pub struct Timeouts {
    pub connect_short: Duration,
    pub connect_long: Duration,
    /// Total time for listings, thumbnails and probes.
    pub short_total: Duration,
    /// How long an export may stay silent before its first byte. The tablet
    /// renders the whole PDF before it starts sending.
    pub first_byte: Duration,
    /// Longest gap between received chunks once an export is flowing.
    pub idle: Duration,
    pub upload_response: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            connect_short: Duration::from_secs(3),
            connect_long: Duration::from_secs(5),
            short_total: Duration::from_secs(10),
            first_byte: Duration::from_secs(15 * 60),
            idle: Duration::from_secs(120),
            upload_response: Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug)]
pub struct SmallResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub struct DownloadStats {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub bytes: u64,
    pub first_byte: Duration,
    pub total: Duration,
    /// First bytes of the body, for signature checks.
    pub head: Vec<u8>,
}

pub struct DeviceTransport {
    short: Client,
    long: Client,
    gate: Mutex<()>,
    timeouts: Timeouts,
}

/// Held while a multi-request sequence must not be interleaved with anything
/// else, such as list root → upload → re-list.
pub struct Session<'a> {
    t: &'a DeviceTransport,
    _guard: MutexGuard<'a, ()>,
}

fn base_url() -> String {
    format!("http://{TABLET_IP}")
}

fn check_path(path: &str) -> Result<()> {
    let ok = path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains("..")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'));
    if ok {
        Ok(())
    } else {
        Err(Error::InvalidPath(path.chars().take(80).collect()))
    }
}

fn headers_of(resp: &Response) -> Vec<(String, String)> {
    resp.headers()
        .iter()
        .map(|(k, v)| (k.to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
        .collect()
}

impl DeviceTransport {
    pub fn new(iface: &TabletInterface, timeouts: Timeouts) -> Result<Self> {
        let build = |connect: Duration| {
            let b = Client::builder()
                .no_proxy()
                .redirect(redirect::Policy::none())
                .connect_timeout(connect)
                .local_address(IpAddr::V4(iface.host_addr))
                .user_agent("PaperBridge");
            #[cfg(target_os = "linux")]
            let b = b.interface(&iface.name);
            b.build()
        };
        Ok(DeviceTransport {
            short: build(timeouts.connect_short)?,
            long: build(timeouts.connect_long)?,
            gate: Mutex::new(()),
            timeouts,
        })
    }

    /// Takes the device queue. Every request goes through a session.
    pub async fn session(&self) -> Session<'_> {
        Session {
            t: self,
            _guard: self.gate.lock().await,
        }
    }
}

impl Session<'_> {
    /// A small request (listing, thumbnail) with a total time limit and a
    /// body size cap.
    pub async fn small(&mut self, method: Method, path: &str, cap: u64) -> Result<SmallResponse> {
        check_path(path)?;
        let limit = self.t.timeouts.short_total;
        let started = Instant::now();
        let work = async {
            let mut resp = self
                .t
                .short
                .request(method, format!("{}{path}", base_url()))
                .send()
                .await?;
            let status = resp.status().as_u16();
            let headers = headers_of(&resp);
            let body = read_capped(&mut resp, cap).await?;
            Ok::<_, Error>(SmallResponse {
                status,
                headers,
                body,
                elapsed: started.elapsed(),
            })
        };
        tokio::time::timeout(limit, work)
            .await
            .map_err(|_| Error::Timeout(limit, "waiting for a short device response"))?
    }

    /// Streams an export to `dest`, which must not exist yet. A non-2xx
    /// response writes nothing and returns the status with a short body head.
    pub async fn download(&mut self, path: &str, dest: &Path, cap: u64) -> Result<DownloadStats> {
        check_path(path)?;
        let t = &self.t.timeouts;
        let started = Instant::now();
        let mut resp = tokio::time::timeout(
            t.first_byte,
            self.t.long.get(format!("{}{path}", base_url())).send(),
        )
        .await
        .map_err(|_| Error::Timeout(t.first_byte, "waiting for the tablet to start an export"))??;
        let first_byte = started.elapsed();
        let status = resp.status().as_u16();
        let headers = headers_of(&resp);

        if !resp.status().is_success() {
            let head = read_capped(&mut resp, 4096).await.unwrap_or_default();
            return Ok(DownloadStats {
                status,
                headers,
                bytes: 0,
                first_byte,
                total: started.elapsed(),
                head,
            });
        }

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .await?;
        let mut bytes = 0u64;
        let mut head = Vec::new();
        let result: Result<()> = async {
            loop {
                let chunk = tokio::time::timeout(t.idle, resp.chunk())
                    .await
                    .map_err(|_| Error::Timeout(t.idle, "receiving an export"))??;
                let Some(chunk) = chunk else { break };
                bytes += chunk.len() as u64;
                if bytes > cap {
                    return Err(Error::TooLarge(cap));
                }
                if head.len() < 16 {
                    let need = 16 - head.len();
                    head.extend_from_slice(&chunk[..chunk.len().min(need)]);
                }
                file.write_all(&chunk).await?;
            }
            file.sync_all().await?;
            Ok(())
        }
        .await;
        if let Err(e) = result {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(e);
        }
        Ok(DownloadStats {
            status,
            headers,
            bytes,
            first_byte,
            total: started.elapsed(),
            head,
        })
    }

    /// Uploads one file. The tablet puts it in the folder that was listed
    /// last, so the caller lists the destination first within this session.
    /// Never retried.
    pub async fn upload(
        &mut self,
        file_name: &str,
        mime: &str,
        data: Vec<u8>,
    ) -> Result<SmallResponse> {
        let limit = self.t.timeouts.upload_response;
        let started = Instant::now();
        let part = reqwest::multipart::Part::bytes(data)
            .file_name(file_name.to_string())
            .mime_str(mime)?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let work = async {
            let mut resp = self
                .t
                .long
                .post(format!("{}/upload", base_url()))
                .header("Origin", base_url())
                .header("Referer", format!("{}/", base_url()))
                .multipart(form)
                .send()
                .await?;
            let status = resp.status().as_u16();
            let headers = headers_of(&resp);
            let body = read_capped(&mut resp, 1 << 20).await?;
            Ok::<_, Error>(SmallResponse {
                status,
                headers,
                body,
                elapsed: started.elapsed(),
            })
        };
        tokio::time::timeout(limit, work)
            .await
            .map_err(|_| Error::Timeout(limit, "uploading a file"))?
    }
}

async fn read_capped(resp: &mut Response, cap: u64) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        if (body.len() + chunk.len()) as u64 > cap {
            return Err(Error::TooLarge(cap));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        for ok in [
            "/documents/",
            "/upload",
            "/download/0a1b2c3d-0000-4111-8222-abcdefabcdef/pdf",
        ] {
            assert!(check_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "documents/",
            "//evil.example/x",
            "/a/../b",
            "/a?b=c",
            "/a b",
            "/a@evil.example",
            "/a#frag",
            "/a%2e%2e",
            "",
        ] {
            assert!(check_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn base_is_fixed() {
        assert_eq!(base_url(), "http://10.11.99.1");
    }
}
