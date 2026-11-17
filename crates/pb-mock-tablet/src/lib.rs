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
    /// Exports stay silent this long before answering, like the tablet
    /// rendering a large notebook. Listings are unaffected.
    Render(u64),
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
            "render" => Fault::Render(4000),
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

/// A small valid one-page PDF, padded so it arrives in several chunks.
pub fn mock_pdf() -> Vec<u8> {
    let text = "BT /F1 24 Tf 72 720 Td (PaperBridge mock export) Tj ET";
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{text}\nendstream", text.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    // Padding comment lines, so progress reporting has something to show.
    for _ in 0..4000 {
        out.extend_from_slice(b"% padding padding padding padding padding padding padding\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes(),
    );
    out
}

fn zip_of(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data, stored) in entries {
        let opts = SimpleFileOptions::default().compression_method(if *stored {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        });
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// A small valid EPUB 3 with two chapters. Chapter 1 carries an inline
/// script and an event handler that must never run in the app, and a link
/// to the internet that must not be followed.
pub fn mock_epub() -> Vec<u8> {
    let container = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="uid">urn:uuid:00000000-0000-4000-8000-00000000e9b0</dc:identifier>
    <dc:title>Mock book</dc:title><dc:language>en</dc:language>
    <meta property="dcterms:modified">2000-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="css" href="style.css" media-type="text/css"/>
    <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml" properties="scripted"/>
    <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>"#;
    let nav = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head>
<body><nav epub:type="toc"><ol><li><a href="c1.xhtml">Chapter one</a></li><li><a href="c2.xhtml">Chapter two</a></li></ol></nav></body></html>"#;
    let c1 = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Chapter one</title><link rel="stylesheet" href="style.css"/>
<script>try { parent.document.body.setAttribute("data-epub-script", "ran") } catch (e) {}</script></head>
<body onload="parent.document.body.setAttribute('data-epub-onload', 'ran')"><h1 class="chapter">Chapter one</h1>
<p class="marker">PaperBridge mock book, first chapter.</p>
<p><a id="external" href="https://example.com/">A link to the internet</a></p></body></html>"#;
    let c2 = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Chapter two</title><link rel="stylesheet" href="style.css"/></head>
<body><h1 class="chapter">Chapter two</h1><p>The second chapter.</p></body></html>"#;
    zip_of(&[
        ("mimetype", b"application/epub+zip", true),
        ("META-INF/container.xml", container, false),
        ("OEBPS/content.opf", opf, false),
        ("OEBPS/nav.xhtml", nav, false),
        ("OEBPS/style.css", b"h1.chapter { color: #2f5d8a; }", false),
        ("OEBPS/c1.xhtml", c1, false),
        ("OEBPS/c2.xhtml", c2, false),
    ])
}

/// A `.rmdoc` shaped like the tablet's: metadata files, plus the original
/// document and the tablet's PDF for imported books.
pub fn mock_rmdoc(id: &str, file_type: &str) -> Vec<u8> {
    let content = format!(r#"{{"fileType":"{file_type}","formatVersion":1,"pageCount":1}}"#);
    let metadata = br#"{"type":"DocumentType","visibleName":"Mock"}"#;
    let pdf = mock_pdf();
    let epub = mock_epub();
    let (n_content, n_meta, n_pdf, n_epub) =
        (format!("{id}.content"), format!("{id}.metadata"), format!("{id}.pdf"), format!("{id}.epub"));
    let mut entries: Vec<(&str, &[u8], bool)> =
        vec![(&n_meta, metadata.as_slice(), false), (&n_content, content.as_bytes(), false)];
    match file_type {
        "pdf" => entries.push((&n_pdf, &pdf, false)),
        "epub" => {
            entries.push((&n_pdf, &pdf, false));
            entries.push((&n_epub, &epub, false));
        }
        _ => {}
    }
    zip_of(&entries)
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
    // Simulates writing on a document: bumps its modified time.
    if let Some(id) = path.strip_prefix("/__mock/touch/") {
        let found = s.items.iter_mut().find(|i| i.get("ID").and_then(Value::as_str) == Some(id));
        return match found {
            Some(item) => {
                item["ModifiedClient"] = json!("2001-01-01T00:00:00.0Z");
                (Reply::Full(200, "text/plain", b"ok".to_vec()), 0)
            }
            None => (Reply::Full(404, "text/plain", b"no such item".to_vec()), 0),
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
        ("GET", ["download", _, "pdf" | "placeholder"]) => match s.fault {
            Fault::Malformed => Reply::Full(200, "text/html", b"<html>error</html>".to_vec()),
            Fault::Render(ms) => return (Reply::Full(200, "application/pdf", mock_pdf()), ms),
            _ => Reply::Full(200, "application/pdf", mock_pdf()),
        },
        ("GET", ["download", id, "rmdoc"]) => match s.fault {
            Fault::Malformed => Reply::Full(200, "application/zip", b"PK\x03\x04mock".to_vec()),
            fault => {
                let file_type = s
                    .items
                    .iter()
                    .find(|i| i.get("ID").and_then(Value::as_str) == Some(id))
                    .and_then(|i| i.get("fileType").and_then(Value::as_str))
                    .unwrap_or("notebook")
                    .to_string();
                let body = mock_rmdoc(id, &file_type);
                let delay = if let Fault::Render(ms) = fault { ms } else { 0 };
                return (Reply::Full(200, "application/zip", body), delay);
            }
        },
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
