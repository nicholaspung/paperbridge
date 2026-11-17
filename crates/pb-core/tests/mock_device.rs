//! pb-core against the mock tablet, including the failures the real tablet
//! and a bad cable can produce.

use pb_core::device::{classify, Device};
use pb_core::listing::DocKind;
use pb_core::transport::{Target, Timeouts};
use pb_core::{ConnectionState, Error};
use pb_mock_tablet::{Fault, MockTablet};
use std::path::Path;
use std::time::Duration;

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/3.29.1.0"))
}

fn fast() -> Timeouts {
    Timeouts {
        short_total: Duration::from_millis(700),
        ..Timeouts::default()
    }
}

async fn setup() -> (MockTablet, Device) {
    let mock = MockTablet::start(fixtures(), 0).await.unwrap();
    let device = Device::new(&Target::Mock(mock.addr), fast()).unwrap();
    (mock, device)
}

#[tokio::test]
async fn walks_the_whole_library() {
    let (mock, device) = setup().await;
    let all = device.walk().await.unwrap();
    assert_eq!(all.len(), 72);
    assert_eq!(all.iter().filter(|r| r.kind == DocKind::Folder).count(), 11);
    assert_eq!(all.iter().filter(|r| r.parent.is_none()).count(), 18);
    // The walk must end on the root so an upload would land there.
    assert_eq!(mock.requests().last().unwrap(), "GET /documents/");
    // And it never asks for a thumbnail.
    assert!(mock.requests().iter().all(|r| !r.contains("thumbnail")));
}

#[tokio::test]
async fn malformed_json_is_unsupported_response() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Malformed);
    let err = device.list(None).await.unwrap_err();
    assert!(matches!(classify(&err), ConnectionState::UnsupportedResponse { .. }));
}

#[tokio::test]
async fn http_errors_are_unsupported_response() {
    let (mock, device) = setup().await;
    for code in [404, 500] {
        mock.set_fault(Fault::Http(code));
        let err = device.list(None).await.unwrap_err();
        assert!(matches!(classify(&err), ConnectionState::UnsupportedResponse { .. }));
    }
}

#[tokio::test]
async fn silent_server_is_reported_as_hung_and_recovers() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Hang);
    let err = device.list(None).await.unwrap_err();
    assert_eq!(classify(&err), ConnectionState::WebInterfaceHung);
    mock.reset();
    assert_eq!(device.list(None).await.unwrap().len(), 18);
}

#[tokio::test]
async fn slow_responses_within_the_limit_succeed() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Slow(300));
    assert_eq!(device.list(None).await.unwrap().len(), 18);
}

#[tokio::test]
async fn truncated_body_is_an_error_not_a_short_listing() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Truncated);
    assert!(device.list(None).await.is_err());
}

#[tokio::test]
async fn oversized_listing_is_refused() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Oversized);
    let err = device.list(None).await.unwrap_err();
    assert!(matches!(err, Error::TooLarge(_)), "{err}");
}

#[tokio::test]
async fn closed_port_means_web_interface_off() {
    let (mock, _) = setup().await;
    // A loopback port with nothing listening.
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let device = Device::new(&Target::Mock(free), fast()).unwrap();
    let err = device.list(None).await.unwrap_err();
    assert_eq!(classify(&err), ConnectionState::WebInterfaceOff);
    drop(mock);
}

#[tokio::test]
async fn mock_target_must_be_loopback() {
    let addr = "10.11.99.1:80".parse().unwrap();
    assert!(Device::new(&Target::Mock(addr), fast()).is_err());
}

#[tokio::test]
async fn requests_are_serialized() {
    let (mock, device) = setup().await;
    mock.set_fault(Fault::Slow(150));
    let started = std::time::Instant::now();
    let (a, b, c) = tokio::join!(device.list(None), device.list(None), device.list(None));
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    // Three 150 ms requests one at a time take at least 450 ms.
    assert!(started.elapsed() >= Duration::from_millis(440), "{:?}", started.elapsed());
}

// ---- Exports ----

use pb_core::device::ExportFormat;
use pb_core::DocId;

fn export_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pb-export-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn leftovers(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

async fn first_document(device: &Device) -> DocId {
    let all = device.walk().await.unwrap();
    all.into_iter().find(|r| r.kind == DocKind::Pdf).unwrap().id
}

#[tokio::test]
async fn exports_a_pdf_atomically_with_progress() {
    let (mock, device) = setup().await;
    let id = first_document(&device).await;
    let dir = export_dir("ok");
    let dest = dir.join("Doc.pdf");
    let mut seen = Vec::new();
    let out = device
        .export(&id, ExportFormat::Pdf, &dest, 1 << 30, &mut |b| seen.push(b))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), pb_mock_tablet::mock_pdf());
    assert_eq!(out.bytes, pb_mock_tablet::mock_pdf().len() as u64);
    assert_eq!(seen.first(), Some(&0), "the first callback marks the first byte");
    assert_eq!(seen.last(), Some(&out.bytes));
    assert_eq!(leftovers(&dir), vec!["Doc.pdf"]);
    assert!(mock.requests().last().unwrap().ends_with("/pdf"));

    // A second export to the same name must not replace the first.
    let err = device.export(&id, ExportFormat::Pdf, &dest, 1 << 30, &mut |_| {}).await.unwrap_err();
    assert!(matches!(err, Error::Io(ref e) if e.kind() == std::io::ErrorKind::AlreadyExists), "{err}");
    assert_eq!(leftovers(&dir), vec!["Doc.pdf"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn rejects_an_export_that_is_not_a_pdf() {
    let (mock, device) = setup().await;
    let id = first_document(&device).await;
    let dir = export_dir("bad");
    mock.set_fault(Fault::Malformed);
    let err = device
        .export(&id, ExportFormat::Pdf, &dir.join("Doc.pdf"), 1 << 30, &mut |_| {})
        .await
        .unwrap_err();
    assert!(matches!(err, Error::BadExport(_)), "{err}");
    assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn export_cap_and_http_errors_leave_nothing() {
    let (mock, device) = setup().await;
    let id = first_document(&device).await;
    let dir = export_dir("cap");
    let err = device.export(&id, ExportFormat::Pdf, &dir.join("a.pdf"), 1000, &mut |_| {}).await.unwrap_err();
    assert!(matches!(err, Error::TooLarge(1000)), "{err}");
    mock.set_fault(Fault::Http(500));
    let err = device.export(&id, ExportFormat::Pdf, &dir.join("a.pdf"), 1 << 30, &mut |_| {}).await.unwrap_err();
    assert!(matches!(err, Error::UnsupportedResponse(_)), "{err}");
    assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn first_byte_timeout_and_cancel_leave_nothing() {
    let mock = MockTablet::start(fixtures(), 0).await.unwrap();
    let device = Device::new(
        &Target::Mock(mock.addr),
        Timeouts { first_byte: Duration::from_millis(500), ..fast() },
    )
    .unwrap();
    let id = first_document(&device).await;
    let dir = export_dir("slow");
    mock.set_fault(Fault::Render(3000));
    let err = device.export(&id, ExportFormat::Pdf, &dir.join("a.pdf"), 1 << 30, &mut |_| {}).await.unwrap_err();
    assert!(matches!(err, Error::Timeout(..)), "{err}");
    assert!(matches!(classify(&err), ConnectionState::WebInterfaceHung));

    // Cancelling is dropping the future.
    let (dest, mut noop) = (dir.join("b.pdf"), |_| {});
    let fut = device.export(&id, ExportFormat::Pdf, &dest, 1 << 30, &mut noop);
    assert!(tokio::time::timeout(Duration::from_millis(200), fut).await.is_err());
    assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn exports_an_rmdoc_and_extracts_the_original_epub() {
    let (_mock, device) = setup().await;
    let all = device.walk().await.unwrap();
    let id = all.into_iter().find(|r| r.kind == DocKind::Epub).unwrap().id;
    let dir = export_dir("epub");
    let archive = dir.join("a.rmdoc");
    device.export(&id, ExportFormat::Rmdoc, &archive, 1 << 30, &mut |_| {}).await.unwrap();
    let dest = dir.join("Book.epub");
    pb_core::rmdoc::extract_original(&archive, &id, "epub", &dest, &Default::default()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), pb_mock_tablet::mock_epub());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn finds_annotated_pages_from_an_rmdoc() {
    let (_mock, device) = setup().await;
    let all = device.walk().await.unwrap();
    let id = all.into_iter().find(|r| r.kind == DocKind::Pdf).unwrap().id;
    let dir = export_dir("ann");
    let archive = dir.join("a.rmdoc");
    device.export(&id, ExportFormat::Rmdoc, &archive, 1 << 30, &mut |_| {}).await.unwrap();
    let ann = pb_core::rmdoc::annotated_pages(&archive, &id, &Default::default()).unwrap();
    assert_eq!((ann.total, ann.pages), (pb_mock_tablet::MOCK_PAGES, vec![2]));
    std::fs::remove_dir_all(&dir).unwrap();
}
