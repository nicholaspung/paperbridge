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
