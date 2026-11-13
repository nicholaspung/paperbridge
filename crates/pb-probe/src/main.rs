//! pb-probe: Phase 0 discovery against a real reMarkable over USB.
//!
//! Read-only unless `--allow-upload` is given. Writes sanitized fixtures and
//! a Markdown report. Titles and IDs never reach those files.

mod sanitize;

use clap::Parser;
use pb_core::platform::{find_tablet_interfaces, TabletInterface};
use pb_core::transport::{DeviceTransport, Session, SmallResponse, Target, Timeouts};
use pb_core::DocId;
use pb_core::transport::Method;
use sanitize::Sanitizer;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;


const LISTING_CAP: u64 = 10 << 20;
const THUMB_CAP: u64 = 5 << 20;
const EXPORT_CAP: u64 = 4 << 30;
const UPLOAD_LIMIT: u64 = 100 << 20;

#[derive(Parser)]
#[command(version, about = "Probe a reMarkable tablet's USB web interface (read-only by default)")]
struct Args {
    /// Firmware version shown on the tablet under Settings > General > Software.
    #[arg(long, default_value = "unknown")]
    firmware: String,
    /// Output directory for the report, fixtures and exported files.
    #[arg(long, default_value = "probe-output")]
    out: PathBuf,
    /// Print real titles and IDs to the terminal (never written to disk), to pick an --export-id.
    #[arg(long)]
    list_titles: bool,
    /// ID of a DISPOSABLE document to export as PDF and .rmdoc.
    #[arg(long)]
    export_id: Option<String>,
    /// Upload this DISPOSABLE PDF/EPUB to the tablet's root folder. This writes to the tablet.
    #[arg(long, value_name = "FILE")]
    allow_upload: Option<PathBuf>,
    /// Also fetch one thumbnail. On firmware 3.x this request has hung the tablet's web server.
    #[arg(long)]
    thumbnail: bool,
    /// Proceed even if the interface could not be confirmed as a reMarkable USB device.
    #[arg(long)]
    accept_unverified_interface: bool,
    /// Stop walking after this many folders.
    #[arg(long, default_value_t = 500)]
    max_folders: usize,
}

struct Item {
    id: Option<DocId>,
    is_folder: bool,
    title: String,
    kind: String,
}

fn items_of(body: &[u8]) -> Option<(Vec<Value>, Vec<Item>)> {
    let Value::Array(arr) = serde_json::from_slice::<Value>(body).ok()? else {
        return None;
    };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let items = arr
        .iter()
        .map(|v| Item {
            id: DocId::parse(&s(v, "ID")).ok(),
            is_folder: s(v, "Type") == "CollectionType",
            title: [s(v, "VissibleName"), s(v, "VisibleName")]
                .into_iter()
                .find(|t| !t.is_empty())
                .unwrap_or_default(),
            kind: format!("{}/{}", s(v, "Type"), s(v, "fileType")),
        })
        .collect();
    Some((arr, items))
}

struct Report {
    md: String,
    san: Sanitizer,
}

impl Report {
    fn line(&mut self, s: impl AsRef<str>) {
        self.md.push_str(s.as_ref());
        self.md.push('\n');
    }

    fn request(&mut self, method: &str, path: &str, r: &pb_core::error::Result<SmallResponse>) {
        let path = self.san.path(path);
        match r {
            Ok(r) => {
                let ct = header(&r.headers, "content-type");
                self.line(format!(
                    "| `{method} {path}` | {} | {} ms | {} | {} |",
                    r.status,
                    r.elapsed.as_millis(),
                    ct,
                    r.body.len()
                ));
            }
            Err(e) => self.line(format!("| `{method} {path}` | error | | | {e} |")),
        }
    }
}

fn header(h: &[(String, String)], name: &str) -> String {
    h.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "-".into())
}

fn magic(head: &[u8]) -> &'static str {
    if head.starts_with(b"%PDF-") {
        "PDF"
    } else if head.starts_with(b"PK\x03\x04") {
        "ZIP"
    } else if head.starts_with(b"\x89PNG") {
        "PNG"
    } else if head.starts_with(b"\xff\xd8") {
        "JPEG"
    } else if head.is_empty() {
        "empty"
    } else {
        "unknown"
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("pb-probe: {e}");
            ExitCode::from(1)
        }
    }
}

fn pick_interface(args: &Args) -> Result<TabletInterface, ExitCode> {
    let found = find_tablet_interfaces().unwrap_or_default();
    let Some(iface) = found.into_iter().next() else {
        eprintln!(
            "No reMarkable USB network interface found. Nothing was sent.\n\
             Check: cable plugged in, tablet awake and unlocked, and\n\
             Settings > Storage > USB web interface switched on."
        );
        return Err(ExitCode::from(2));
    };
    println!(
        "Interface {}: host {} netmask {} usb {:?}",
        iface.name, iface.host_addr, iface.netmask, iface.usb
    );
    if !iface.is_known_tablet() && !args.accept_unverified_interface {
        eprintln!(
            "This interface could not be confirmed as a reMarkable USB device, so nothing was sent.\n\
             If you are sure it is the tablet, rerun with --accept-unverified-interface."
        );
        return Err(ExitCode::from(3));
    }
    Ok(iface)
}

async fn run(args: Args) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let export_id = args.export_id.as_deref().map(DocId::parse).transpose()?;
    let iface = match pick_interface(&args) {
        Ok(i) => i,
        Err(code) => return Ok(code),
    };
    let transport = DeviceTransport::new(&Target::Tablet(iface.clone()), Timeouts::default())?;
    let mut s = transport.session().await;

    let fixtures = args.out.join("fixtures");
    let raw = args.out.join("raw");
    std::fs::create_dir_all(&fixtures)?;
    std::fs::create_dir_all(&raw)?;

    let mut rep = Report {
        md: String::new(),
        san: Sanitizer::default(),
    };
    rep.line("# pb-probe report\n");
    rep.line(format!("- Firmware (as entered): `{}`", args.firmware));
    rep.line(format!("- Host OS: `{}`", std::env::consts::OS));
    rep.line(format!(
        "- Interface: `{}`, host address `{}`, netmask `{}`",
        iface.name, iface.host_addr, iface.netmask
    ));
    match &iface.usb {
        Some(u) => rep.line(format!(
            "- USB: `{:04x}:{:04x}` {:?} {:?}",
            u.vendor, u.product, u.manufacturer, u.product_name
        )),
        None => rep.line("- USB: not verified on this OS"),
    }
    rep.line("\n## Requests\n\n| Request | Status | Time | Content-Type | Bytes / error |\n|---|---|---|---|---|");

    // Root listing, both methods.
    let get = s.small(Method::GET, "/documents/", LISTING_CAP).await;
    rep.request("GET", "/documents/", &get);
    let post = s.small(Method::POST, "/documents/", LISTING_CAP).await;
    rep.request("POST", "/documents/", &post);

    let parsed = |r: &pb_core::error::Result<SmallResponse>| {
        r.as_ref()
            .ok()
            .filter(|r| r.status == 200)
            .and_then(|r| items_of(&r.body))
    };
    let (method, root) = match (parsed(&get), parsed(&post)) {
        (Some(root), _) => (Method::GET, root),
        (None, Some(root)) => (Method::POST, root),
        (None, None) => {
            rep.line("\nNeither listing method returned a JSON array. Stopping.");
            std::fs::write(args.out.join("report.md"), &rep.md)?;
            eprintln!(
                "The interface is up but the web interface did not return a listing.\n\
                 If the request was refused, switch on Settings > Storage > USB web interface.\n\
                 If it timed out, wake and unlock the tablet. See {}/report.md.",
                args.out.display()
            );
            return Ok(ExitCode::from(4));
        }
    };
    let root_headers = get.as_ref().or(post.as_ref()).map(|r| r.headers.clone()).unwrap_or_default();

    // Walk folders.
    let mut all_raw: Vec<Value> = Vec::new();
    let mut all: Vec<Item> = Vec::new();
    let mut queue: VecDeque<DocId> = VecDeque::new();
    let mut listings = vec![("documents_root".to_string(), root.0.clone())];
    let mut seen = BTreeSet::new();
    let mut take = |(rawv, items): (Vec<Value>, Vec<Item>), queue: &mut VecDeque<DocId>| {
        for it in &items {
            if let (true, Some(id)) = (it.is_folder, &it.id) {
                if seen.insert(id.clone()) {
                    queue.push_back(id.clone());
                }
            }
        }
        all_raw.extend(rawv);
        all.extend(items);
    };
    take(root, &mut queue);
    let mut folders = 0;
    while let Some(id) = queue.pop_front() {
        if folders >= args.max_folders {
            rep.line(format!("\nStopped walking at {} folders.", args.max_folders));
            break;
        }
        folders += 1;
        let path = format!("/documents/{id}");
        let r = s.small(method.clone(), &path, LISTING_CAP).await;
        rep.request(method.as_str(), &path, &r);
        if let Some(children) = parsed(&r) {
            listings.push((format!("documents_folder_{folders:03}"), children.0.clone()));
            take(children, &mut queue);
        }
    }

    if args.list_titles {
        println!("\n{:<38} {:<28} TITLE", "ID", "TYPE");
        for it in &all {
            let id = it.id.as_ref().map(|i| i.to_string()).unwrap_or_else(|| "(invalid id)".into());
            println!("{id:<38} {:<28} {}", it.kind, it.title);
        }
    }

    // Thumbnail of the first document.
    let first_doc = all.iter().filter(|i| !i.is_folder).find_map(|i| i.id.clone());
    if let (true, Some(id)) = (args.thumbnail, first_doc) {
        let path = format!("/thumbnail/{id}");
        let r = s.small(Method::GET, &path, THUMB_CAP).await;
        rep.request("GET", &path, &r);
        if let Ok(r) = &r {
            rep.line(format!("| ↳ thumbnail signature | {} | | | |", magic(&r.body)));
        }
    }

    // Fixtures.
    for (name, rawv) in &listings {
        let clean: Vec<Value> = rawv.iter().map(|v| rep.san.value(v)).collect();
        std::fs::write(
            fixtures.join(format!("{name}.json")),
            serde_json::to_string_pretty(&clean)? + "\n",
        )?;
    }

    // Inventory.
    rep.line("\n## Root listing response headers\n");
    for (k, v) in &root_headers {
        rep.line(format!("- `{k}: {v}`"));
    }
    rep.line(format!("\nListing method used for the walk: `{method}`"));
    rep.line(format!(
        "\n## Library shape\n\n- Items: {}\n- Folders walked: {folders}\n- Items with an ID that is not a UUID: {}",
        all.len(),
        all.iter().filter(|i| i.id.is_none()).count()
    ));
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for it in &all {
        *kinds.entry(it.kind.as_str()).or_default() += 1;
    }
    for (k, n) in kinds {
        rep.line(format!("- `Type/fileType` = `{k}`: {n}"));
    }
    rep.line("\n## Fields seen\n\n| Field | JSON types | Example (safe fields only) |\n|---|---|---|");
    for (k, (types, example)) in sanitize::inventory(&all_raw) {
        let types: Vec<_> = types.into_iter().collect();
        rep.line(format!("| `{k}` | {} | {} |", types.join(", "), example));
    }

    // Exports.
    if let Some(id) = &export_id {
        rep.line("\n## Exports\n\n| Path | Status | First byte | Total | Bytes | Signature | Content-Type | Content-Disposition present |\n|---|---|---|---|---|---|---|---|");
        for (suffix, ext) in [("pdf", "pdf"), ("placeholder", "placeholder.pdf"), ("rmdoc", "rmdoc")] {
            let path = format!("/download/{id}/{suffix}");
            let dest = raw.join(format!("export.{ext}"));
            let _ = std::fs::remove_file(&dest);
            println!("Exporting {suffix} (the tablet may take minutes to render)...");
            let shown = rep.san.path(&path);
            match s.download(&path, &dest, EXPORT_CAP).await {
                Ok(d) => rep.line(format!(
                    "| `{shown}` | {} | {} ms | {} ms | {} | {} | {} | {} |",
                    d.status,
                    d.first_byte.as_millis(),
                    d.total.as_millis(),
                    d.bytes,
                    magic(&d.head),
                    header(&d.headers, "content-type"),
                    header(&d.headers, "content-disposition") != "-"
                )),
                Err(e) => rep.line(format!("| `{shown}` | error: {e} | | | | | | |")),
            }
        }
    } else {
        rep.line("\n## Exports\n\nSkipped: no `--export-id` given.");
    }

    // Upload.
    if let Some(file) = &args.allow_upload {
        upload(&mut s, &mut rep, file, method.clone()).await?;
    } else {
        rep.line("\n## Upload\n\nSkipped: no `--allow-upload` given.");
    }

    rep.line("\n## To check by hand\n\n- [ ] Does the web interface stay reachable while the tablet sleeps?\n- [ ] Is the USB web interface setting still on after a reboot?\n- [ ] Was the uploaded test file placed in the root folder on the tablet?");
    std::fs::write(args.out.join("report.md"), &rep.md)?;
    println!(
        "\nDone. Report: {0}/report.md  Fixtures: {0}/fixtures/  Raw exports (do not commit): {0}/raw/",
        args.out.display()
    );
    Ok(ExitCode::SUCCESS)
}

async fn upload(
    s: &mut Session<'_>,
    rep: &mut Report,
    file: &std::path::Path,
    method: Method,
) -> Result<(), Box<dyn std::error::Error>> {
    rep.line("\n## Upload\n");
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mime = match ext.as_str() {
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        _ => return Err("only .pdf and .epub files can be uploaded".into()),
    };
    let data = std::fs::read(file)?;
    if data.len() as u64 > UPLOAD_LIMIT {
        return Err("file is over the 100 MB USB upload limit".into());
    }
    let ids = |r: &SmallResponse| -> BTreeSet<String> {
        items_of(&r.body)
            .map(|(_, items)| items.into_iter().filter_map(|i| i.id.map(|d| d.to_string())).collect())
            .unwrap_or_default()
    };
    // List root → upload → list root, all inside this one session.
    let before = s.small(method.clone(), "/documents/", LISTING_CAP).await?;
    let name = format!("pb-probe-test.{ext}");
    let up = s.upload(&name, mime, data).await;
    let after = s.small(method, "/documents/", LISTING_CAP).await?;
    match &up {
        Ok(r) => {
            rep.line(format!("- Response: status {}, {} ms", r.status, r.elapsed.as_millis()));
            for (k, v) in &r.headers {
                rep.line(format!("  - `{k}: {v}`"));
            }
            let body = serde_json::from_slice::<Value>(&r.body)
                .map(|v| rep.san.value(&v).to_string())
                .unwrap_or_else(|_| format!("<{} bytes, not JSON>", r.body.len()));
            rep.line(format!("- Body (sanitized): `{body}`"));
        }
        Err(e) => rep.line(format!("- Upload request failed: {e}")),
    }
    let new: Vec<_> = ids(&after).difference(&ids(&before)).cloned().collect();
    let mut note = String::new();
    write!(note, "- New IDs in the root folder after upload: {}", new.len())?;
    if new.len() != 1 {
        note.push_str(" (expected exactly 1; check on the tablet where the file went)");
    }
    rep.line(note);
    rep.line("- Sent with `Origin` and `Referer` set to the tablet address, multipart field `file`.");
    Ok(())
}
