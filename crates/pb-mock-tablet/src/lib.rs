//! A stand-in for the reMarkable USB web interface, for tests.
//!
//! Hand-written HTTP/1.1 over a raw socket so that faults (truncated bodies,
//! silence, oversized replies) can be produced exactly. Loopback only.
//!
//! Faithful to firmware 3.29.1.0 in one unpleasant way: a thumbnail request
//! gets no reply and leaves the whole server silent until `reset`.

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    /// Delay every response by this many milliseconds.
    Slow(u64),
    /// Listings return broken JSON.
    Malformed,
    /// Every request returns this status.
    Http(u16),
    /// Listings promise a full body and send half of it.
    Truncated,
    /// Accept connections, never answer.
    Hang,
    /// Listings return more than the client's 10 MB cap.
    Oversized,
}

impl Fault {
    pub fn parse(s: &str) -> Option<Fault> {
        Some(match s {
            "none" => Fault::None,
            "malformed" => Fault::Malformed,
            "truncated" => Fault::Truncated,
            "hang" => Fault::Hang,
            "oversized" => Fault::Oversized,
            "http500" => Fault::Http(500),
            "http404" => Fault::Http(404),
            "slow" => Fault::Slow(1500),
            _ => return None,
        })
    }
}

struct State {
    items: Vec<Value>,
    fault: Fault,
    hung: bool,
    requests: Vec<String>,
    uploads: usize,
}

#[derive(Clone)]
pub struct MockTablet {
    pub addr: SocketAddr,
    state: Arc<Mutex<State>>,
}

impl MockTablet {
    /// Loads every `*.json` listing in `fixtures` and serves on `127.0.0.1:port`
    /// (0 picks a free port).
    pub async fn start(fixtures: &Path, port: u16) -> std::io::Result<MockTablet> {
        let mut items: Vec<Value> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut paths: Vec<_> = std::fs::read_dir(fixtures)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let parsed: Value = serde_json::from_slice(&std::fs::read(&path)?)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            for item in parsed.as_array().cloned().unwrap_or_default() {
                let id = item.get("ID").and_then(Value::as_str).unwrap_or("").to_string();
                if seen.insert(id) {
                    items.push(item);
                }
            }
        }
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let mock = MockTablet {
            addr: listener.local_addr()?,
            state: Arc::new(Mutex::new(State {
                items,
                fault: Fault::None,
                hung: false,
                requests: Vec::new(),
                uploads: 0,
            })),
        };
        let state = mock.state.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, state.clone()));
            }
        });
        Ok(mock)
    }

    pub fn set_fault(&self, fault: Fault) {
        self.state.lock().unwrap().fault = fault;
    }

    /// Clears the fault and the hung state, like toggling the USB web interface.
    pub fn reset(&self) {
        let mut s = self.state.lock().unwrap();
        s.fault = Fault::None;
        s.hung = false;
    }

    /// Request lines received so far, such as `GET /documents/`.
    pub fn requests(&self) -> Vec<String> {
        self.state.lock().unwrap().requests.clone()
    }

    pub fn item_count(&self) -> usize {
        self.state.lock().unwrap().items.len()
    }
}

enum Reply {
    Full(u16, &'static str, Vec<u8>),
    /// Declare the full length, send only part, then close.
    Partial(Vec<u8>),
    Silence,
}

fn children(items: &[Value], parent: &str) -> Vec<u8> {
    let kids: Vec<&Value> = items
        .iter()
        .filter(|i| i.get("Parent").and_then(Value::as_str).unwrap_or("") == parent)
        .collect();
    serde_json::to_vec(&kids).unwrap()
}

fn route(state: &Mutex<State>, method: &str, path: &str) -> (Reply, u64) {
    let mut s = state.lock().unwrap();
    s.requests.push(format!("{method} {path}"));

    if let Some(name) = path.strip_prefix("/__mock/fault/") {
        return match Fault::parse(name) {
            Some(f) => {
                s.fault = f;
                s.hung = false;
                (Reply::Full(200, "text/plain", b"ok".to_vec()), 0)
            }
            None => (Reply::Full(400, "text/plain", b"unknown fault".to_vec()), 0),
        };
    }
    if s.hung || s.fault == Fault::Hang {
        return (Reply::Silence, 0);
    }
    let delay = match s.fault {
        Fault::Slow(ms) => ms,
        _ => 0,
    };
    if let Fault::Http(code) = s.fault {
        return (Reply::Full(code, "text/plain", b"error".to_vec()), delay);
    }
    const JSON: &str = "application/json; charset=ISO-8859-1";
    let listing = |s: &State, parent: &str| match s.fault {
        Fault::Malformed => Reply::Full(200, JSON, b"[{\"ID\": ".to_vec()),
        Fault::Truncated => Reply::Partial(children(&s.items, parent)),
        Fault::Oversized => {
            let mut body = vec![b' '; 11 << 20];
            body[0] = b'[';
            *body.last_mut().unwrap() = b']';
            Reply::Full(200, JSON, body)
        }
        _ => Reply::Full(200, JSON, children(&s.items, parent)),
    };
    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
    let reply = match (method, segs.as_slice()) {
        ("GET" | "POST", ["documents"]) => listing(&s, ""),
        ("GET" | "POST", ["documents", id]) => listing(&s, id),
        ("GET", ["thumbnail", _]) => {
            s.hung = true;
            Reply::Silence
        }
        ("GET", ["download", _, "pdf" | "placeholder"]) => {
            Reply::Full(200, "application/pdf", b"%PDF-1.4\n%mock\n%%EOF\n".to_vec())
        }
        ("GET", ["download", _, "rmdoc"]) => {
            Reply::Full(200, "application/zip", b"PK\x03\x04mock".to_vec())
        }
        ("POST", ["upload"]) => {
            s.uploads += 1;
            let n = s.uploads;
            s.items.push(json!({
                "Bookmarked": false, "CurrentPage": 0,
                "ID": format!("ffffffff-0000-4000-8000-{n:012x}"),
                "ModifiedClient": "2000-01-01T00:00:00.0Z", "Parent": "",
                "Type": "DocumentType", "VisibleName": format!("Upload {n}"),
                "VissibleName": format!("Upload {n}"), "fileType": "pdf"
            }));
            Reply::Full(201, JSON, br#"{"status":"Upload successful"}"#.to_vec())
        }
        _ => Reply::Full(404, "text/plain", b"not found".to_vec()),
    };
    (reply, delay)
}

async fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>) {
    // Read the head, then drain any body so uploads complete.
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_string();
    let path = first.next().unwrap_or("").to_string();
    let content_length: usize = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    let mut remaining = content_length.saturating_sub(buf.len() - head_end);
    while remaining > 0 {
        match stream.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => remaining = remaining.saturating_sub(n),
        }
    }

    let (reply, delay) = route(&state, &method, &path);
    if delay > 0 {
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    let head = |code: u16, ctype: &str, len: usize| {
        format!(
            "HTTP/1.1 {code} X\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
        )
    };
    match reply {
        Reply::Full(code, ctype, body) => {
            let _ = stream.write_all(head(code, ctype, body.len()).as_bytes()).await;
            let _ = stream.write_all(&body).await;
        }
        Reply::Partial(body) => {
            let _ = stream
                .write_all(head(200, "application/json", body.len().max(2)).as_bytes())
                .await;
            let _ = stream.write_all(&body[..body.len() / 2]).await;
        }
        Reply::Silence => {
            // Hold the connection open, as the real tablet does.
            let _ = stream.read(&mut tmp).await;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
    let _ = stream.shutdown().await;
}
