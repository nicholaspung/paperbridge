//! Usage: pb-mock-tablet <fixtures-dir> [port]
//! Switch faults at run time with `GET /__mock/fault/<name>`
//! (none, slow, malformed, truncated, hang, oversized, http500, http404).

use pb_mock_tablet::MockTablet;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let fixtures = PathBuf::from(args.next().expect("usage: pb-mock-tablet <fixtures-dir> [port]"));
    let port: u16 = args.next().map(|p| p.parse().expect("port")).unwrap_or(0);
    let mock = MockTablet::start(&fixtures, port).await?;
    println!("pb-mock-tablet listening on http://{} ({} items)", mock.addr, mock.item_count());
    std::future::pending::<()>().await;
    Ok(())
}
