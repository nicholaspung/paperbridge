# PaperBridge

A USB-only desktop manager for the reMarkable 2, for Linux and macOS. No cloud account, no reMarkable desktop app, and no internet traffic from the app.

**Status: Phase 0.** Only the device transport (`pb-core`) and the discovery tool (`pb-probe`) exist. The desktop app is not built yet. See [the plan](PaperBridge_Implementation_Plan.md).

## pb-probe

`pb-probe` talks to the tablet's USB web interface at `10.11.99.1` and records how this firmware behaves. It is read-only unless you pass `--allow-upload`. It sends nothing unless it finds a reMarkable USB network interface.

On the tablet: plug in the cable, unlock it, and switch on Settings → Storage → USB web interface.

```
cargo build --release -p pb-probe

# 1. Read-only pass. Prints real titles and IDs to the terminal only.
./target/release/pb-probe --firmware 3.x.y.z --list-titles

# 2. Export a disposable document as PDF and .rmdoc.
./target/release/pb-probe --firmware 3.x.y.z --export-id <ID from step 1>

# 3. Upload a disposable file to the root folder. This writes to the tablet.
./target/release/pb-probe --firmware 3.x.y.z --export-id <ID> --allow-upload sample.epub
```

Output goes to `probe-output/` (git-ignored):

- `report.md`: what worked, with timings. Titles and IDs are replaced.
- `fixtures/`: sanitized listing JSON, to be copied to `tests/fixtures/<firmware>/` after review.
- `raw/`: the exported files. These are your documents; do not commit them.

On macOS the tool cannot yet confirm that the interface is a USB device, so it asks for `--accept-unverified-interface`.

## Development

Rust stable via `rustup`. `cargo test --workspace` runs everything and needs no tablet.
