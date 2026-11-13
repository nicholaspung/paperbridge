# PaperBridge — Implementation Plan

**Version:** 1.4 · **Date:** 2026-11-12 · **Target:** Linux + macOS desktop, reMarkable 2 · **Primary dev environment:** Linux (Arch) · **Status:** specification for a coding agent

## 0. Changes

**v1.4 (Phase 1 as built):**
- **No thumbnails and no `pbthumb://` scheme.** The endpoint is unsafe (v1.3.1), so the library shows type icons and the CSP allows no custom scheme.
- **The library is walked in full on connect** (12 requests, well under a second on the owner's tablet) and cached in SQLite; the UI only ever reads the cache. The walk ends by listing the root.
- **Connection states changed:** `WebInterfaceUnreachable` is split into `WebInterfaceOff` (port 80 refused) and `WebInterfaceHung` (accepts, never answers).
- **Linux privacy check:** a reMarkable USB interface without a tablet-assigned address is the warning signal (it is what NetworkManager's "shared" mode produces). NAT rules are not inspected.
- **Deferred from Phase 1:** device profiles for more than one tablet (§6), USB verification of the interface on macOS, and macOS host privacy checks. The app says "unverified" on macOS and accepts a subnet match there.
- **Linux/NVIDIA/Wayland:** the app disables WebKitGTK's DMA-BUF renderer for itself to avoid a startup crash (`docs/PLATFORM_NOTES.md`).
- TypeScript types are generated with `ts-rs` and committed; CI fails if they are stale.

**v1.3.1 (first hardware results, firmware 3.29.1.0; details in `docs/API_COMPATIBILITY.md`):**
- **Do not call `GET /thumbnail/{id}`.** The one request made got no reply and hung the tablet's web server until the USB web interface was toggled. Thumbnails are out of the MVP unless a safe source is found (for example, rendering page 1 of an exported PDF on the host).
- Listing works with `GET` and `POST`; `/pdf`, `/placeholder` and `/rmdoc` all work. The listing has both title spellings and no device, firmware, or size fields.
- The host address and netmask changed after a firmware update (`10.11.99.11/27` → `10.11.99.2/29`).
- New connection-state cases: port 80 **refused** means the web interface is off; port 80 **accepting but silent** means the web server is hung and the user must toggle the setting.

**v1.3 (review pass):**
- **Export timeouts** gain a separate time-to-first-byte allowance, because the tablet renders before it sends (§4.1).
- **Uploads hold the device queue** across list-root → upload → verify, and verification diffs document IDs (§4.1).
- **`mock-device` feature** also fakes interface detection, is blocked from release builds by `compile_error!`, and the release binary is checked in CI (§4.1, §10.1).
- **USB precondition:** no request is sent unless the interface is a USB device, and the socket is bound to that interface's address (§4.2).
- **macOS Local Network behavior** is confirmed with the first CI-built `.app` in Phase 1, not with `pb-probe`, because a CLI inherits Terminal's permission (§3.2, §9).
- **Backups are self-describing:** JSON manifests in the backup folder are the source of truth, SQLite is a rebuildable index, and documents are stored as plain named copies (§3.1, §7.3).
- **Read-only SSH diagnostics move up to Phase 3 (v1.0)**, which is what makes the "verified" privacy state reachable. **Basic host-side privacy checks move down to Phase 1.**
- **Device identity** is a user-named profile; firmware is entered by hand or read over SSH (§6).
- **Library is served from cache during transfers** (§5).
- **Phase 0 is its own assignment** and ends before the mock and app are built (§11).
- Smaller fixes: CI step order, no hard-link fallback on exFAT, IP forwarding alone is informational, `reqwest` built without TLS.
- **Observed on the owner's machine (Arch, NetworkManager):** USB ID `04b3:4010`, driver `cdc_ether`, host address `10.11.99.11` by DHCP, NetworkManager auto-creates a wired connection. With the tablet unplugged, `10.11.99.1` routes to the Wi-Fi gateway.

**v1.2:**
- **Personal macOS builds come from a GitHub-hosted macOS runner** by default (§3.3), so the Mac needs no Rust toolchain.
- A local toolchain on the Mac is optional, for iterating quickly on Mac-only problems.
- The same workflow builds and tests Linux on every push.

**v1.1 (from v1.0):**

- **Cross-platform.** Linux and macOS are both first-class targets. Development happens primarily on Linux; macOS is used for macOS builds, signing, and macOS-specific checks. Windows is not a target, but avoid choices that would rule it out.
- **Rust workspace split.** Device, parsing, transfer, and backup logic live in a GUI-free `pb-core` crate. A `pb-probe` CLI (Phase 0 and field diagnostics) and a `pb-mock-tablet` server sit beside the Tauri app.
- **Per-OS host networking and privacy checks** (§4.2, §7.2), including the macOS 15+ Local Network permission.
- **Testing strategy rewritten** (§10.1). Real WebDriver end-to-end tests run on Linux. macOS has no WebDriver for WKWebView, so macOS gets a manual smoke checklist.
- **Transport hardening made concrete.** Per-operation timeouts, one in-flight device request, proxies and redirects disabled, and a test-only base-URL override that is compiled out of release builds.
- **Known device JSON quirks and ID validation** written into the contract.
- **Cross-platform file safety.** Filename sanitization, Unicode normalization, case-insensitive uniqueness, and support for backups on external exFAT drives.
- **Scope fix.** "Basic preview" in the MVP now has a home in Phase 2.

## 1. Goal and non-negotiable requirements

Build a polished cross-platform desktop app (Linux and macOS) that manages a reMarkable 2 over a **USB cable only**. It must work without the official desktop app, a reMarkable cloud account, or a reMarkable Connect subscription. It must import PDF/EPUB files, browse the tablet library, export annotated PDFs and native `.rmdoc` archives, and keep versioned local backups. An optional later feature can pull reading material from Readwise Reader **on the host computer** and send it to the tablet by USB.

**Privacy boundaries:**
- In the default configuration, PaperBridge itself must initiate **no outbound internet traffic**. That rules out telemetry, analytics, crash upload, automatic update checks, CDN fonts, remote UI resources, and reMarkable API access.
- It must use only the tablet's USB interface at `http://10.11.99.1`. An explicitly enabled advanced mode may also use SSH to that same USB address.
- The host computer may stay online. The tablet must have Wi-Fi off and must not be routed to the internet over USB.
- PaperBridge must never claim the tablet is isolated unless it can verify the relevant conditions.
- Optional Readwise fetching, if enabled later, is a separately consented internet operation on the host. It is not a tablet connection.

**Data-integrity boundary:** Reading, exports, and backups are safe defaults. Do not write directly to the tablet's proprietary notebook/document filesystem in v1. Do not silently delete, overwrite, rename, reorganize, or restore tablet notebooks.

## 2. Scope by release

| Capability | MVP (v0.1) | v1.0 | Later / research |
|---|---|---|---|
| Platforms | Linux x86_64 (AppImage/.deb) and macOS Apple Silicon dev builds | Signed + notarized macOS `.dmg`; Linux AppImage/.deb with checksums | Windows, Linux arm64, Flatpak, macOS Intel/universal |
| USB connect/disconnect and status | Yes | Yes | — |
| Browse tablet folders, documents, thumbnails | Yes | Yes | — |
| Search/filter local document listing | Yes | Yes | Indexed text search |
| Drag-and-drop PDF/EPUB import | Yes (root folder only) | Yes | OS share-menu integration |
| Export annotated PDF | Yes | Yes | Layer-aware custom renderer |
| Export complete native `.rmdoc` | Yes | Yes | Native parser and editable restore tooling |
| Multi-document/folder recursive bulk export | Yes | Yes | — |
| Manual backup to a user-selected directory | Yes | Yes | — |
| Incremental, versioned backups on connect | No | Yes | — |
| Preview backed-up PDF, manage offline library | Basic (open in system viewer) | In-app browser of backups | — |
| Privacy/network-isolation diagnostics | Basic host-side checks (Phase 1); status is "unverified" or "warning" only | Full diagnostics; "verified" available with SSH | Optional OS-level network blocking |
| SSH access | No | Optional read-only diagnostics (Phase 3) | Advanced device controls |
| Rename/move/delete/create folders on tablet | No | No | Only after careful, tested implementation |
| Readwise Reader article import | No | Optional v1.1 | Bidirectional highlighting/status sync |
| Full tablet OS snapshots, templates, firmware, mirroring | No | No | Possible advanced modules |

**Not in scope:** cloning reMarkable's desktop application; cloud syncing; internet access on the tablet; editing handwritten strokes on the computer; guaranteed preservation of every unsupported proprietary feature; running an Android Reader app.

## 3. Technology stack

### 3.1 Decisions

- **Desktop shell:** Tauri 2.
  - Webviews: WKWebView on macOS, WebKitGTK 4.1 (`webkit2gtk-4.1`) on Linux. Both are WebKit, but rendering and font differences still exist, so test the UI on both.
- **Frontend:** Svelte 5 + TypeScript + Vite. Plain Vite, not SvelteKit: the app has a handful of views and needs no router or SSR.
- **Rust backend:** a Cargo workspace (see §6).
  - All device HTTP traffic and filesystem writes happen in Rust, never through a webview `fetch`.
  - Do **not** enable `tauri-plugin-http` or `tauri-plugin-shell`.
- **HTTP client:** `reqwest` with `default-features = false` and **no TLS feature**. The device speaks plain HTTP, so the binary carries no TLS stack until Readwise (Phase 4) adds `rustls-tls` for a separate client.
  - The device client must be built with `.no_proxy()` and `redirect::Policy::none()`.
- **Persistent metadata:** SQLite through `rusqlite` with the `bundled` feature, so the SQLite version is the same on every OS. Store device profiles, document metadata, import history, hashes, errors, and known firmware/API capabilities there. Keep actual document content as native files.
  - SQLite is an **index of** the backup manifests, not their home. The source of truth is a JSON manifest per snapshot inside the backup folder (§7.3), and the index can be rebuilt from those files.
- **Paths:** use Tauri's path API / the `directories` crate. Never hard-code locations.

  | Purpose | Linux | macOS |
  |---|---|---|
  | Settings + database | `$XDG_DATA_HOME/paperbridge` (default `~/.local/share/paperbridge`) | `~/Library/Application Support/PaperBridge` |
  | Cache (thumbnails, temp) | `$XDG_CACHE_HOME/paperbridge` | `~/Library/Caches/PaperBridge` |
  | Default backup folder | `<XDG Documents dir>/PaperBridge Backups` (fall back to `~/PaperBridge Backups`) | `~/Documents/PaperBridge Backups` |

  The backup folder is always user-selectable and may be on an external drive.
- **Secrets:** only needed once SSH or Readwise credentials exist. Use the `keyring` crate: macOS Keychain on macOS, Secret Service (GNOME Keyring / KWallet) on Linux. If no Secret Service is available, do **not** fall back to a plaintext file. Ask for the credential each session instead. Never write tokens or passwords to logs.
- **Distribution:**
  - macOS: signed and notarized `.dmg`, Apple Silicon first. **It must be built on macOS.** Tauri cannot cross-compile macOS bundles from Linux, so use the developer's Mac or a macOS CI runner.
  - Linux: AppImage and `.deb` (`.rpm` optional), built on the oldest supported Ubuntu LTS (22.04) for glibc compatibility, published with SHA-256 checksums and optionally a GPG signature.
  - No updater plugin and no mandatory online license on either platform.
- **CI:** GitHub Actions (§3.3). Linux tests and builds run on every push; the macOS build runs on demand and is the default way to get personal macOS builds. CI is a developer-side service and does not affect the app's runtime network policy.

Rationale: Tauri gives one codebase for both platforms with a small footprint. SwiftUI no longer fits because Linux is a target. Electron would work, but it ships Chromium and is heavier for no benefit here.

### 3.2 Development environment

**Linux (primary).** The owner's machine is Arch (Omarchy): `webkit2gtk-4.1` is already installed and includes `WebKitWebDriver`; Rust comes from a user-level `rustup`. Local Arch builds are for development only. The distributable AppImage/`.deb` come from the Ubuntu CI job. Debian/Ubuntu packages, as used in CI:
```
sudo apt install build-essential curl wget file libxdo-dev libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  webkit2gtk-driver xvfb
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Node LTS + pnpm
cargo install tauri-driver --locked
```

**macOS:** no toolchain needed by default. Personal builds of the app and `pb-probe` come from CI (§3.3), and the Mac is used only to run them.
- **Optional local toolchain:** if Mac-only problems need fast iteration (for example the Local Network permission), install Xcode Command Line Tools, `rustup` (user-level, removable with `rustup self uninstall`), and Node LTS + pnpm. Then use `pnpm tauri dev` / `pnpm tauri build` on the Mac.
- **Public releases (v1.0):** signing and notarization need an Apple Developer Program membership. They can run in CI using repository secrets or on the Mac.

**Hardware:** the tablet can be connected to either machine. Phase 0 discovery runs on Linux with `pb-probe`. On macOS, the CI-built `pb-probe` confirms basic connectivity only: a CLI started from Terminal runs under **Terminal's** Local Network permission, so it says nothing about the app's own prompt. The app's prompt and denial behavior are confirmed with the first CI-built `.app` in Phase 1.

### 3.3 CI builds (GitHub Actions)

A single workflow, `.github/workflows/build.yml`, with two jobs:

| Job | Runs on | Trigger | Does |
|---|---|---|---|
| `linux` | `ubuntu-22.04` | every push / pull request, and manual | Rust + UI tests, Linux e2e under `xvfb`, AppImage + `.deb` |
| `macos` | `macos-latest` (Apple Silicon) | **manual only** (`workflow_dispatch`) | `cargo test`, `.dmg`, and a `pb-probe` binary |

macOS runs only when triggered by hand because macOS minutes cost much more than Linux minutes on private repos (free on public repos; check current GitHub pricing). The Rust cache keeps repeat builds well under the first build's time, which is roughly 10–15 minutes.

Starting point. Pin each action to its **current** major version when creating the file, and adjust script names to match `package.json`:

```yaml
name: Build
on:
  push:
  pull_request:
  workflow_dispatch:          # "Run workflow" button; the only trigger for the macOS job

permissions:
  contents: read

jobs:
  linux:
    runs-on: ubuntu-22.04     # oldest supported LTS, for glibc compatibility
    steps:
      - uses: actions/checkout@v7
      - name: System packages
        run: |
          sudo apt-get update
          sudo apt-get install -y build-essential libxdo-dev libssl-dev \
            libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
            webkit2gtk-driver xvfb
      - uses: pnpm/action-setup@v4          # reads "packageManager" from package.json
      - uses: actions/setup-node@v4
        with: { node-version: lts/*, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: pnpm install --frozen-lockfile
      - run: pnpm build                      # frontend dist must exist before src-tauri compiles
      - run: cargo test --workspace
      - run: pnpm test                       # Vitest UI tests
      - run: cargo install tauri-driver --locked
      - run: pnpm build:e2e                  # debug app with --features mock-device
      - run: xvfb-run -a pnpm test:e2e       # WebdriverIO + tauri-driver vs pb-mock-tablet
      - run: pnpm tauri build --bundles appimage,deb
      - run: pnpm check:release-override     # release binary must ignore PB_DEVICE_BASE_URL (§4.1)
      - uses: actions/upload-artifact@v7
        with:
          name: PaperBridge-linux
          path: |
            target/release/bundle/appimage/*.AppImage
            target/release/bundle/deb/*.deb
          retention-days: 7

  macos:
    if: github.event_name == 'workflow_dispatch'
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v7
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with: { node-version: lts/*, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: pnpm install --frozen-lockfile
      - run: pnpm build
      - run: cargo test --workspace
      - run: pnpm tauri build --bundles dmg
      - run: cargo build --release -p pb-probe
      - uses: actions/upload-artifact@v7
        with:
          name: PaperBridge-macos
          path: |
            target/release/bundle/dmg/*.dmg
            target/release/pb-probe
          retention-days: 7
```

Paths use the root `target/` directory because the project is a Cargo workspace (§6). Until the Tauri app exists (Phase 0), the workflow only runs `cargo test --workspace` and builds `pb-probe`. When creating the file, confirm the `ubuntu-22.04` runner image is still offered; if it has been retired, use the oldest available LTS and update the supported-distro line in §10.2.

**Personal (unsigned) macOS builds:**
- Set `"bundle": { "macOS": { "signingIdentity": "-" } }` in `tauri.conf.json`. This gives the app an ad-hoc signature, which Apple Silicon requires before it will run code. The Rust linker already ad-hoc signs the `pb-probe` binary.
- Download the artifact from the run page in the Actions tab (or with `gh run download`).
- macOS quarantines downloaded unsigned apps. Clear it once per download:
  ```
  xattr -dr com.apple.quarantine /Applications/PaperBridge.app
  xattr -d com.apple.quarantine ./pb-probe && chmod +x ./pb-probe
  ```
  Alternatively, try to open the app, then use System Settings → Privacy & Security → Open Anyway.
- The ad-hoc signature changes with every build, so macOS may ask for Local Network permission again after each new download, or keep a stale decision. If access seems blocked, reset the permission in System Settings → Privacy & Security → Local Network. Confirm this behavior in Phase 0 and record it in `docs/PLATFORM_NOTES.md`.

**Public releases (v1.0, Phase 3):** add a separate, tag-triggered release job that signs with a Developer ID certificate and notarizes, using repository secrets. Never commit certificates or Apple credentials. Until then, the workflow above produces personal builds only.

## 4. Device integration contract

### 4.1 USB HTTP interface

The tablet exposes a USB network interface at the documented address `10.11.99.1`. Users enable the USB web interface in the tablet's Storage settings. The official interface supports PDF/EPUB/`.rmdoc` transfer, with a documented 100 MB USB upload limit. The HTTP endpoints are **community-documented, not a guaranteed stable public SDK**. Verify every operation against the actual reMarkable 2 firmware before depending on it.

| Action | Candidate endpoint | Caveat |
|---|---|---|
| List root folder | `GET` **or** `POST /documents/` | Community tools use both methods; confirm in Phase 0. Validate schema. |
| List children | `GET`/`POST /documents/{id}` | Folder IDs are opaque; never infer from names |
| Export annotated PDF | `GET /download/{id}/pdf` | Some firmware uses `/download/{id}/placeholder` instead |
| Export native archive | `GET /download/{id}/rmdoc` | Only on sufficiently recent firmware (documented from 3.9); confirm |
| Import file | `POST /upload` (multipart) | Requires appropriate Origin/Referer; destination is the last folder listed (stateful) |
| Thumbnail | `GET /thumbnail/{id}` | **Hung the web server on 3.29.1.0. Do not call.** Show a placeholder |

**Known response quirks to normalize** (confirm against fixtures): the display-name field is spelled `VissibleName`; folders vs. documents are distinguished by `Type` = `CollectionType` / `DocumentType`; `fileType` distinguishes `pdf` / `epub` / notebook; timestamps appear as `ModifiedClient`; `Parent` holds the parent ID. Treat every field as optional, and map unknown values to `unknown` instead of failing the whole listing.

**ID validation:** document and folder IDs must match a strict UUID pattern before they are placed into any URL path. Reject anything else as `InvalidDocumentId`.

**Upload caveat:** community documentation says uploads land in the **last folder listed by the USB service**; there is no explicit destination parameter. This is global mutable state. Rules for v0.1:
- Advertise imports to the root folder only.
- Serialize all imports.
- Treat **list root → upload → re-list root** as one critical section that holds the device queue. No other request (folder browsing, thumbnails) may run in between, because any listing changes the upload destination.
- Verify by **diffing the root's document-ID set** before and after: exactly one new ID whose title matches the uploaded file. Titles alone are not enough because duplicates are allowed. If the new ID is not in the root, report where it landed and stop the import queue.
- Never advertise reliable import to arbitrary folders until that behavior is conclusively tested. Another browser or client could interfere.

**Transport rules** (`DeviceTransport` in `pb-core`):
- **One in-flight request to the device at a time.** The tablet is slow and stateful. Thumbnail fetches go through the same queue at lower priority and are dropped when no longer visible.
- **Timeouts per operation class:**

  | Class | Connect | Total / idle |
  |---|---|---|
  | Probe, listing | 3 s | 10 s total |
  | Thumbnail | 3 s | 10 s total |
  | PDF / `.rmdoc` export | 5 s | No total cap; **15 min to first byte**; then 120 s **idle** timeout between received bytes; user-cancelable |
  | Upload | 5 s | Idle timeout on send; 5 min for the response |

  The tablet renders an annotated PDF **before it sends the first byte**, so a large document can be silent for minutes. The idle timeout therefore applies only once bytes are flowing, and the wait for the first byte has its own, much longer limit with visible "tablet is rendering" progress and a cancel button. Never use a single short global timeout. Make the values configurable and tune them from Phase 0 measurements.
- **Retries:** only idempotent GETs/list calls, with bounded exponential backoff. Never retry an upload automatically. Re-list and verify instead.
- **Size caps:** listing ≤ 10 MB, thumbnail ≤ 5 MB. Exports stream directly to a temp file, with a configurable cap and a disk-space check.
- **Content-type and signature checks:** PDFs must start with `%PDF-`; `.rmdoc` must be a valid ZIP (see §7.3).
- **No proxies and no redirects.**
- **Allowlist:** the release build accepts only `http://10.11.99.1`. A base-URL override (`PB_DEVICE_BASE_URL`) exists **only** when compiled with the `mock-device` cargo feature. Even then it accepts only loopback addresses.
  - The `mock-device` feature also replaces interface detection with a fake "USB interface up" result. Without that, the app stops at `NoUsbInterface` and never reaches the mock.
  - Cargo features are independent of profiles, so enforce the rule in code: `#[cfg(all(feature = "mock-device", not(debug_assertions)))] compile_error!(...)`.
  - A CI step runs the **release binary** with `PB_DEVICE_BASE_URL` pointing at a loopback listener and fails if the listener receives a connection.
- **USB precondition:** before any request, the interface that owns the route to `10.11.99.1` must be a USB network device (§4.2), and the client socket is bound to that interface's local address. If either check fails, send nothing.

**Connection states** (explicit enum, not strings):
`NoUsbInterface` → `UsbInterfaceUp` (probing) → `Connected`, plus the failure states `LocalNetworkPermissionDenied` (macOS), `WebInterfaceUnreachable` (interface up but no HTTP: web interface disabled, or tablet asleep/locked), `UnsupportedResponse`, and `TransportError`.

**Device quirks to surface in the UI:**
- The web interface disappears while the tablet sleeps or is locked.
- On some firmware the USB web interface setting may reset after a reboot. Confirm in Phase 0.

**Optional SSH integration:** deferred to a later phase. On the reMarkable 2, SSH over USB can provide read-only firmware/device details and network-route diagnostics.
- Authentication must use a pinned host key and keyring-stored credentials.
- A root shell is high privilege. In v1, do not expose an arbitrary terminal, execute downloaded scripts, or write proprietary storage files.
- Never build commands from unsanitized document names.

### 4.2 Host USB networking (per OS)

The tablet presents a USB Ethernet gadget (RNDIS/CDC-ECM). The host gets an address in `10.11.99.x` from the tablet over DHCP. Record the exact host address and netmask in Phase 0.

**Detection (both OSes):** enumerate network interfaces (for example with the `if-addrs` crate) and look for an IPv4 address in the tablet's subnet **on an interface that is a USB device**.
- A subnet match alone is not enough. On a VPN or LAN inside `10.0.0.0/8`, or simply with the tablet unplugged, `10.11.99.1` routes somewhere else (observed: to the Wi-Fi gateway). Probing or uploading there would break the privacy boundary.
- USB check on Linux: `/sys/class/net/<if>/device/subsystem` resolves to `usb`, and the USB vendor/product is `04b3:4010`. Treat an unknown ID on a USB interface as "possibly a tablet" and ask before use; record any new IDs in `docs/DEVICE_COMPATIBILITY.md`.
- USB check on macOS: resolve the interface through SystemConfiguration/IOKit and confirm a USB parent with the same IDs.
- If no such interface exists, report `NoUsbInterface` without touching the network.
- If one exists, bind to its address and probe `10.11.99.1`.
- While disconnected, poll the interface list every few seconds. Never scan LAN addresses.

**Linux:**
- Requires the `cdc_ether` / `rndis_host` kernel modules (present on mainstream distros).
- NetworkManager normally configures the interface as a DHCP client automatically. Interface names vary (`usb0`, `enx…`, `enp…u…`). Observed on the owner's Arch machine: driver `cdc_ether`, interface `enp0s20f0u5c2`, auto-created "Wired connection" profile, host address `10.11.99.11`.
- There is no permission prompt.
- Troubleshooting docs should cover: interface missing (module or cable), NetworkManager marking the device unmanaged, and the NetworkManager connection being set to **"Shared to other computers"**. That last case is internet sharing and must trigger a privacy warning.

**macOS:**
- The tablet appears as a network service, for example "RNDIS/Ethernet Gadget".
- **macOS 15+ Local Network privacy:** the app must declare `NSLocalNetworkUsageDescription` in `Info.plist`. The user is prompted on first access. If they deny it, connections to `10.11.99.1` fail even though the interface is up; expect "no route to host" or similar, and confirm in Phase 0.
- Map that failure (interface present + host unreachable) to `LocalNetworkPermissionDenied`. Show steps to fix it under System Settings → Privacy & Security → Local Network.

## 5. UX and primary user journeys

### Window layout

- **Design:** one consistent PaperBridge design on both OSes. Don't imitate native widgets of either platform. Follow the system light/dark preference. Bundle all fonts locally and load nothing remote.
- **Sidebar:** Tablet (when connected), Local backups, Imports, Activity, Settings.
- **Header:** device name, USB connected/disconnected state, backup status, privacy status (**verified / unverified / warning**, not a misleading green indicator).
- **Library:** folders, PDF/EPUB/notebook distinctions, thumbnails, document title, optional modification time, search by title, single/multi-select.
  - The library is rendered from a **cached listing** (kept in SQLite, refreshed on connect and after each transfer). The device handles one request at a time and an export can take minutes, so while a transfer runs the user browses the cache, sees a "showing cached library, refresh queued" note, and missing thumbnails stay as placeholders until the queue is free.
- **Actions:** `Import PDFs or EPUBs`, `Export annotated PDF`, `Back up native file`, `Back up selected`, `Back up all`, and `Show in folder` (Finder on macOS, the default file manager on Linux, via `tauri-plugin-opener`).
- **Activity drawer:** progress, transfer speeds, item-by-item results, retry/cancel, and errors written in plain language.

### Journey A — first launch

1. Present the privacy policy and explain that the app makes no default internet requests.
2. Ask the user to plug in and unlock the tablet, turn Wi-Fi off, and enable the USB web interface.
3. Platform step:
   - **macOS:** explain the upcoming Local Network prompt and why to allow it.
   - **Linux:** confirm the USB interface appeared.
4. Detect the interface, probe `/documents/`, and show an actionable failure reason if either fails (see connection states).
5. Show the tablet library. Let the user choose a local backup directory. No account registration.

### Journey B — import

1. The user drags one or more `.epub` / `.pdf` files onto the app, or picks them in the system file dialog.
2. Validate extension, file signature, readability, and the documented 100 MB limit. Reject unsupported or corrupt files with an explanation.
3. Queue transfers one at a time to avoid stateful upload races. Show progress and allow cancel before the next item.
4. Refresh the library and confirm each imported document appears. Do not treat HTTP 200 alone as success.
5. Keep the original files on the computer, log local import receipts, and show duplicate warnings instead of silently overwriting.

### Journey C — retrieve annotated material

1. The user selects a document, clicks `Export annotated PDF`, and chooses a directory.
2. Download the tablet-rendered PDF to a temp file in the destination directory. Validate the PDF signature and nonzero size, then move it into place atomically (§7.3).
3. Name the output using the cross-platform naming rules (§7.3). If a file already exists, create a versioned filename or ask for explicit overwrite.
4. Offer `Back up native .rmdoc` separately. Explain that the PDF is the readable annotation export, while `.rmdoc` is needed to preserve reMarkable's native editable document state.
5. Offer `Open` (system default viewer) and `Show in folder`.

### Journey D — backup every time the tablet is connected (v1.0)

1. On connection, load the last manifest and enumerate the library recursively.
2. Compare stable document IDs and **tested** metadata to decide which documents changed. If change detection is unreliable, re-export rather than risk skipping data.
3. Save native `.rmdoc`, and optionally PDF, for every changed document, recursively, in a versioned snapshot with metadata and SHA-256 checksums.
4. Use temp files and atomic commits. Support resuming an interrupted session. Never mark an incomplete transfer as backed up.
5. Keep previous snapshots, including for documents that are missing or deleted on the tablet. Never propagate deletions automatically.
6. Provide restore-readiness checks, but postpone pushing restores back to the tablet until that has been validated against test notebooks and firmware.

## 6. Project layout and contracts

```
paperbridge/
  Cargo.toml                   # workspace
  package.json                 # pnpm; Svelte app
  src/                         # Svelte 5 + TypeScript UI
    views/ DeviceLibrary, Backups, Imports, Activity, Settings, FirstRun
    components/ DocumentGrid, ConnectionBadge, PrivacyBadge, TransferQueue
    state/ device.svelte.ts, transfers.svelte.ts, privacy.svelte.ts
    lib/ipc.ts                 # the only place that calls Tauri invoke(); typed wrappers
  crates/
    pb-core/                   # NO Tauri dependency; builds and tests on any OS
      transport/               # DeviceTransport: allowlist, timeouts, queue, size caps
      device/                  # probe, capability profile, listing, JSON normalization
      documents/               # import, PDF/.rmdoc export, thumbnails
      transfer/                # sequential job queue, retry, cancel, progress events
      backup/                  # recursive walk, snapshots, checksums, manifests
      storage/                 # SQLite, atomic writes, filename sanitization
      platform/                # per-OS: interface detection, route/forwarding/sharing checks (cfg-gated)
      privacy/                 # diagnostics aggregation, honest state labels
    pb-probe/                  # CLI: Phase 0 discovery + user-run field diagnostics; writes sanitized fixtures
    pb-mock-tablet/            # axum server mimicking the USB API, with fault injection
  src-tauri/
    src/commands/              # thin, restricted Tauri command layer over pb-core
    capabilities/              # Tauri 2 capability files: minimal permissions
    Info.plist                 # NSLocalNetworkUsageDescription (macOS)
  tests/
    fixtures/                  # sanitized firmware-specific JSON and sample archives
    e2e/                       # WebdriverIO + tauri-driver (Linux)
  docs/
    DEVICE_COMPATIBILITY.md
    PLATFORM_NOTES.md          # Linux/macOS setup, USB networking, troubleshooting
    PRIVACY_AND_THREAT_MODEL.md
    API_COMPATIBILITY.md
    MANUAL_SMOKE_CHECKLIST.md  # per-OS, esp. macOS
    RELEASE_CHECKLIST.md
```

Design an abstraction instead of binding the UI directly to raw firmware JSON:

- `DeviceIdentity`: a locally generated profile ID, a **user-chosen tablet name**, optional firmware version, session capability flags, connection state.
  - The USB HTTP interface is not known to expose model, serial, or firmware, and the USB serial string is `0`. Phase 0 confirms this.
  - Firmware is typed in by the owner (Settings → General → Software on the tablet) or read over SSH once that exists. `pb-probe` takes it as `--firmware`.
  - On connect, match the tablet to a profile by overlap between its document IDs and the profile's last manifest. Ask the user when there is no clear match, so two tablets never share a backup history.
- `DocumentRecord`: opaque `id` (validated UUID), title, type (`folder`, `pdf`, `epub`, `notebook`, `unknown`), parent ID, optional dates/size/thumbnail.
- `TransferJob`: `jobId`, operation (`import`, `pdf-export`, `rmdoc-backup`), source/destination, status, retries, bytes written, error.
- `BackupManifest`: snapshot ID, timestamp, device identity, document ID, content format, SHA-256, source metadata, local relative path, completeness marker.
- `ConnectionState`, `PrivacyStatus`: explicit enums (§4.1, §7.2).

Use explicit result/error types with semantic error codes, serialized to the UI as tagged unions. Generate the TypeScript types from Rust (for example with `specta`/`tauri-specta` or `ts-rs`) so the two sides cannot drift. Avoid stringly typed exception handling in the UI. Build `pb-mock-tablet` first so most development and tests need no physical device.

**Thumbnails in the UI:** serve cached thumbnails through a custom URI scheme registered in Rust (for example `pbthumb://`). The webview never talks to the tablet, and the CSP only has to allow that scheme.

## 7. Privacy, security, and trust model

### 7.1 App network policy

- The default runtime allowlist is the tablet's USB HTTP endpoint `10.11.99.1:80` only. No reMarkable domains, web login, analytics, telemetry, crash reports, CDNs, embedded remote pages, or automatic update fetches.
- Networking lives in Rust. Untrusted UI content must not be able to choose hosts or run commands.
- Use strict Tauri 2 capabilities and a strict CSP, roughly: `default-src 'self'; img-src 'self' pbthumb: data:; connect-src ipc: http://ipc.localhost; style-src 'self'; font-src 'self'`.
- Disable proxies for the device client (`.no_proxy()`), ignore `HTTP(S)_PROXY`, and confirm the route to `10.11.99.1` goes through the USB interface (§7.2).
- Treat all tablet metadata and document contents as untrusted input.
- If Readwise is added later, it gets an explicit `Enable Readwise` setting. Requests originate from the **host** to Readwise only, the token lives in the OS keyring, and it has its own network policy and test cases.

### 7.2 Tablet network isolation

- First-launch checklist: tablet Wi-Fi off; no reMarkable cloud account required; host internet sharing to the USB interface disabled.
- **Host-side checks** are read-only and never modify system configuration:

  | Check | Linux | macOS |
  |---|---|---|
  | Route to `10.11.99.1` uses the USB interface | netlink / `ip route get 10.11.99.1` | `route -n get 10.11.99.1` |
  | IP forwarding enabled (**informational only**: Docker and libvirt turn it on, so it is not a warning by itself) | `/proc/sys/net/ipv4/ip_forward` | `sysctl net.inet.ip.forwarding` |
  | Internet sharing to the USB interface | NetworkManager connection with `ipv4.method=shared` on that interface | Internet Sharing enabled (confirm in Phase 0 how to read this without privileges) |

  Prefer reading files, sysctls, or D-Bus over running commands. If a command is unavoidable, use a fixed absolute path and fixed arguments, and never include device- or user-provided strings.
- Host checks can produce a **warning** (internet sharing or NAT to the USB interface detected), but they cannot prove isolation. Without an authorized read-only SSH inspection of the tablet's interfaces and routes, the status stays **isolation unverified**. Say so plainly. Never imply the app controls the device OS.
- **Timing:** the route and sharing checks ship in Phase 1, because the route check is also the transport's USB precondition. Until SSH diagnostics ship in Phase 3, the badge has only two states, **unverified** and **warning**. **Verified** appears in v1.0 and only for users who enable SSH.
- Optionally document a stronger posture: a manually reviewed tablet firewall, or a host firewall rule (nftables on Linux, pf on macOS), with recovery instructions, outside the default install. Never silently edit the host's firewall configuration.
- The app's own outbound allowlist is **not equivalent** to device-level network isolation.

### 7.3 File safety

- Validate remote document IDs and URL paths. Defend against path traversal, ZIP-slip, ZIP bombs (cap entry count, total uncompressed size, and compression ratio), malformed JSON, and oversized responses.
- **Cross-platform filename rules** for anything derived from tablet titles:
  - Normalize to Unicode NFC.
  - Strip control characters.
  - Replace `/ \ : * ? " < > |`.
  - Trim trailing dots and spaces.
  - Avoid Windows reserved names (`CON`, `NUL`, …), which is cheap insurance.
  - Cap names at 200 UTF-8 bytes.
  - Ensure uniqueness **case-insensitively**. macOS APFS is case-insensitive by default, and exFAT/FAT external drives are too.
- **Atomic writes:**
  - Write to a temp file in the destination directory, then `fsync` it.
  - Rename without clobbering. Try an atomic no-replace rename (`renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on macOS). Where the filesystem rejects it, do **not** fall back to hard links (`tempfile::persist_noclobber` does, and exFAT has none). Instead check that the target is absent and rename, under the app's single-writer lock for that backup folder. Verify this on a real exFAT drive in Phase 2.
  - `fsync` the directory on Linux. On macOS, use `F_FULLFSYNC` when committing a snapshot.
- **Backups must work on external exFAT/FAT drives.** Do not depend on symlinks, hard links, POSIX permissions, or extended attributes for correctness or deduplication.
- **Backup layout: plain named copies, self-describing.**
  - Each snapshot is a folder of ordinary files named by the rules above, so backups can be browsed and opened without PaperBridge.
  - Each snapshot contains a `manifest.json` (document ID, title, tablet path, format, size, SHA-256, source metadata, completeness marker), written last and atomically. A snapshot without a complete manifest is treated as interrupted.
  - Unchanged documents are not copied again: a later manifest points at the file in the earlier snapshot by relative path and hash. Retention must never delete a file that a kept manifest still references.
  - The backup folder alone is enough to restore the index on a new machine.
- Verify exports before committing them, calculate hashes, and keep historical backups. Never keep originals only as rendered PDFs.
- If backups need encryption, rely on FileVault / LUKS or a user-selected encrypted volume in v1.0 rather than hand-rolled cryptography.
- No remote code execution, software installs, root filesystem changes, or firmware updates in the ordinary workflow.

## 8. Reuse and reverse-engineering boundaries

**RCU (reMarkable Connection Utility):** a feature benchmark, not a code dependency for the initial release. Its documented features include USB/SSH document management, full backups/snapshots, native notebook archives, annotated exports, advanced PDF rendering, custom templates, Markdown, and firmware management. The author distributes source with purchase and licenses RCU under **AGPLv3-or-later**. Studying its interfaces and reproducing behavior independently is fine. Reusing its source directly requires checking license compatibility and honoring AGPL obligations, especially when redistributing a derivative. Do not assume all of its rendering features work on current reMarkable firmware.

**rm-exporter (`chopikus/rm-exporter`):** an MIT-licensed Go/Svelte project that handles recursive exports, retries, and timeouts for large documents over USB. A good reference for backup robustness. Reusing its code requires keeping the MIT notices.

**reMarkable WebUI (`mahiatlinux/remarkable-webui`):** a newer Tauri/Node/SSH-based feature reference, with a rich browser, folder operations, handwriting preview, and screen mirroring. Its repository landing page did **not show a LICENSE file** when checked. Treat it as reference-only until an explicit license or permission exists. Do not copy its code, assets, or UI wholesale.

**reMarkable USB API documentation:** a community reference for understanding wire behavior. Independently capture response schemas from the owner's device and add them as regression fixtures. Never modify proprietary notebook storage based only on unverified documentation.

## 9. Implementation phases and acceptance gates

### Phase 0 — Discovery and non-destructive proof of concept (Linux first)

- Build `pb-core::transport` (minimal) and `pb-probe`. The probe:
  - records the host interface, address, and netmask;
  - refuses to send anything unless the USB precondition holds (§4.2);
  - records the firmware given with `--firmware`, plus any model/firmware/serial hints found in HTTP headers or responses;
  - tries both `GET` and `POST` for listing;
  - walks folders;
  - fetches one thumbnail;
  - exports PDF (`/pdf` and `/placeholder`) and `.rmdoc` for a **disposable** document;
  - measures export latency;
  - performs an upload **only** with the `--allow-upload` flag and a disposable sample file.
- The probe writes sanitized fixtures (titles and IDs replaced) to `tests/fixtures/<firmware>/`, plus a Markdown report. The owner runs it and commits the output.
- Document which PDF path works, the actual `rmdoc` response, upload headers, content limits, target-folder behavior, sleep behavior, and whether the web-interface toggle survives a reboot.
- On macOS, use the CI-built `pb-probe` (§3.3) to confirm connectivity and to find a privilege-free way to read the Internet Sharing state. Local Network prompt behavior is **not** tested here (§3.2); it moves to the Phase 1 gate.
- Do not build production upload features, the mock tablet, or JSON normalization beyond what the probe needs until this evidence exists. The mock and the normalizer are then written from the captured fixtures.

**Gate:** a disposable annotated PDF and native archive can be exported, and a disposable EPUB imported, from Linux, without reMarkable cloud access and without losing existing data. Connectivity is also confirmed from macOS. **Work stops here until the owner has run the probe and the fixtures are committed.**

### Phase 1 — Desktop shell and safe device browser

- Scaffold the workspace (§6): Tauri 2 + Svelte 5 + TypeScript, `pb-core`, `pb-mock-tablet`, and generated TS types.
- Implement interface detection, connection states, the first-run guide (with per-OS steps), device status, listing, folders, thumbnails (custom scheme, placeholder), and search/filter, all working against the mock tablet.
- Basic host-side privacy checks (§7.2): route uses the USB interface, internet sharing/NAT to it. Badge states: unverified or warning.
- Cached library listing in SQLite, so browsing never waits on the device queue (§5).
- No SSH, and no outbound URLs beyond USB.
- Add unit, fixture, mock-integration, UI component, and Linux end-to-end tests (§10.1).
- Extend `.github/workflows/build.yml` (§3.3) with the app build, e2e, and the release-override check.

**Gate:** browsing survives an unplug/replug cycle on **both Linux and macOS**; the macOS check may use a CI-built app. On macOS, the Local Network prompt, a denial, and a re-downloaded build are each exercised with the CI-built `.app` and recorded in `docs/PLATFORM_NOTES.md`. All automated tests pass without the tablet in the Linux CI job, and `cargo test` passes in the macOS CI job.

### Phase 2 — Reliable imports and annotated exports (MVP)

- Implement drag-and-drop (Tauri drag-drop events work on both OSes), an ordered upload queue, progress, verification, PDF and `.rmdoc` exports, and recursive bulk downloads.
- Add cancel, timeouts, retry policy, network-interruption handling, atomic temp-file writes, and cross-platform naming.
- Basic offline library: list exported/backed-up files, `Open` in the system viewer, `Show in folder`.
- Test with PDF, EPUB, notebooks, Unicode and case-colliding titles, duplicate titles, large exports, sleep/disconnect, and a backup folder on an exFAT drive.
- Produce Linux AppImage/.deb and an unsigned macOS dev build.

**Gate:** import, annotate on the tablet, then retrieve the annotated PDF **and native archive** with no integrity loss and no internet route needed, on both Linux and macOS.

### Phase 3 — Versioned backup engine, privacy diagnostics, release builds (v1.0)

- In-folder JSON manifests with a rebuildable SQLite index (§7.3); per-document versions; immutable completed snapshots; re-export strategy; restore-readiness audit.
- Opt-in backup on connect, configurable snapshot retention, periodic verification of stored hashes.
- Full per-OS host privacy diagnostics with honest state labels (§7.2).
- **Optional read-only SSH diagnostics** (off by default, §4.1 rules): pinned host key, credential in the OS keyring, a fixed set of read-only commands. It reads the firmware version and the tablet's interfaces and routes. This is the only path to the **verified** isolation state.
- Automated network audit on Linux (§10.1) and a manual audit on macOS.
- Signed and notarized macOS `.dmg`; Linux packages with checksums; release checklist.

**Gate:** interrupt a backup by unplugging the tablet, reconnect, and complete it without destroying earlier backups, on both OSes. With Wi-Fi disabled and no USB internet sharing, the network audit shows no app-initiated internet traffic during a normal session.

### Phase 4 — Readwise Reader importer (optional v1.1)

- Explicit Readwise opt-in. Authenticate with a token stored in the OS keyring (Keychain / Secret Service). Without a Secret Service, ask for the token per session and never store it in plaintext.
- Fetch user-selected Reader documents/tags through the official Reader API, paginate correctly, and convert HTML to well-formed EPUB, keeping source attribution and images where available.
- Prefer the source EPUB/PDF when available. Queue content for USB import and track Reader IDs in a local ledger.
- Never overwrite a tablet copy that may be annotated when a Readwise item updates. Create a new edition or ask for approval.

**Gate:** save/tag an article in Reader, fetch it on the host, disconnect the host from the internet, and still transfer the queued EPUB over USB with the tablet offline.

### Phase 5 — Advanced options (separate risk/ROI decision)

- SSH beyond the read-only diagnostics of Phase 3.
- Folder moves/renames, template installation, custom PDF rendering, smarter highlight extraction, native notebook restore, or full OS snapshots, **only as independently tested features**.
- Windows support, Linux arm64, Flatpak (needs portal-based file access and a review of network permissions).

## 10. Definition of done and test matrix

### 10.1 Testing strategy

| Layer | Tooling | Runs on |
|---|---|---|
| Core unit + fixture tests | `cargo test -p pb-core` | Linux, macOS (any OS) |
| Device integration | `pb-core` against `pb-mock-tablet` with fault injection: slow responses, truncated bodies, mid-stream disconnects, malformed JSON, HTTP 4xx/5xx, oversized bodies, missing thumbnails, upload landing in the wrong folder | Linux, macOS |
| UI components | Vitest + `@testing-library/svelte`, with Tauri IPC mocked through `@tauri-apps/api/mocks` | Linux, macOS |
| End-to-end | WebdriverIO + `tauri-driver` + `WebKitWebDriver`, under `xvfb` in CI, against the mock tablet | **Linux only** (no WebDriver for WKWebView on macOS) |
| macOS app smoke | `docs/MANUAL_SMOKE_CHECKLIST.md`: launch, Local Network prompt, connect, browse, export, drag-drop, Show in Finder | macOS, every release |
| Network audit | Run the e2e suite under `strace -f -e trace=connect` (or in a network namespace) and fail on any connection that isn't to loopback, the mock, or local Unix sockets (D-Bus/X11/Wayland). This audits the `mock-device` build, so also run the **release** binary idle for 60 s under the same trace | Linux CI |
| Release override check | Start the release binary with `PB_DEVICE_BASE_URL` set to a loopback listener; fail if it connects | Linux CI |
| Network audit (manual) | `nettop` / `lsof -i` or a firewall monitor during the smoke checklist | macOS, every release |
| Hardware validation | `pb-probe` + manual checklist with the real tablet | Linux and macOS, at each phase gate |

### 10.2 Checklist

- [ ] Runs on Linux x86_64 (Ubuntu 22.04+ / current Fedora) and macOS Apple Silicon without the official reMarkable app or a cloud login.
- [ ] Connect/disconnect, sleeping tablet, disabled USB interface, macOS Local Network permission denied, Linux interface missing, and firmware/schema changes all produce actionable error messages.
- [ ] Users can browse folders and tell EPUB/PDF/notebook types apart.
- [ ] PDF/EPUB imports are verified on the device; the 100 MB limit is handled clearly; concurrent uploads never race.
- [ ] Annotated PDF and `.rmdoc` exports are readable, nonempty, and saved atomically, including to exFAT drives.
- [ ] Recursive multi-document backup recovers from disconnects and keeps previous versions, including items deleted from the tablet.
- [ ] No device delete/overwrite/move happens without explicit authorization; v1 does not write proprietary notebook storage.
- [ ] Tests cover path traversal, invalid document IDs, malformed JSON, archive traversal/bombs, HTTP errors, slow exports, duplicate and case-colliding names, Unicode normalization, and missing thumbnails.
- [ ] Release builds reject the base-URL override; the device client ignores proxy environment variables.
- [ ] The network audit shows zero app-initiated public internet connections during default workflows on both OSes. Tablet isolation is reported as verified or unverified, never assumed.
- [ ] macOS build is signed and notarized; Linux packages ship with checksums; dependency licenses are audited (`cargo-deny` / `cargo-about`, plus a pnpm license report).
- [ ] Documentation covers per-OS setup, USB networking troubleshooting, USB-only privacy assumptions, local backup recovery, update compatibility, and failure scenarios.

## 11. Coding-agent assignments (copy/paste)

**Assignment A — Phase 0 tooling. Stop when done.**

> You are implementing PaperBridge, a cross-platform (Linux + macOS), USB-only manager for reMarkable 2. Primary development is on Linux. Read `PaperBridge_Implementation_Plan.md` in full. Implement **Phase 0 tooling only**:
> 1. Create the Cargo workspace from §6 with `pb-core` and `pb-probe` only.
> 2. In `pb-core` (no Tauri dependency): the allowlisted `DeviceTransport` (one in-flight request, per-class timeouts with a separate time-to-first-byte limit, no proxy, no redirects, size caps, USB precondition and bound socket), UUID validation, explicit `ConnectionState`, and Linux + macOS interface detection.
> 3. Build `pb-probe` (read-only by default; upload only with `--allow-upload`) so the owner can capture sanitized fixtures and a report from the real tablet.
> 4. Add a GitHub Actions workflow that runs `cargo test --workspace` on Linux on every push and builds `pb-probe` for macOS on demand.
>
> Then stop. The owner runs the probe on the tablet and commits `tests/fixtures/<firmware>/` and the report.

**Assignment B — Phase 1, after the fixtures exist.**

> Read the plan and the Phase 0 report. Implement **Phase 1 only**:
> 1. Add the pnpm project and the Tauri 2 + Svelte 5 app laid out in §6.
> 2. Write tolerant JSON normalization into `DocumentRecord` and `pb-mock-tablet` (with fault injection) **from the captured fixtures**, not from community documentation alone.
> 3. Add the `mock-device` feature as specified in §4.1, including faked interface detection and the `compile_error!` guard.
> 4. Build the app: first-launch guide with per-OS steps, folder browser backed by the cached listing, thumbnail placeholders via a custom URI scheme, search/filter, basic host privacy checks, strict capabilities and CSP, and `NSLocalNetworkUsageDescription`.
> 5. Add the tests from §10.1 that apply to Phase 1, including Linux e2e with `tauri-driver`, the network audit, and the release-override check.
> 6. Extend the workflow per §3.3, with an on-demand macOS job producing an ad-hoc-signed `.dmg` plus `pb-probe`. The developer's Mac has no Rust toolchain, so macOS builds must come from CI.
>
> No SSH, no cloud endpoints, no telemetry, no updater, no device filesystem writes, and **no production import button until Phase 0 upload validation**. Document unknown firmware behaviors instead of guessing. Provide Linux and macOS build instructions, the macOS manual smoke checklist, and a short hardware-validation checklist before moving to Phase 2.

## 12. References and primary reading

- Official USB transfer and formats: https://support.remarkable.com/articles/Knowledge/importing-and-exporting-files
- Community USB API reference (updated Aug. 20, 2026): https://remarkable.guide/tech/usb-web-interface.html
- Additional USB API research: https://github.com/Scratchydisk/remarkable-rm/blob/main/docs/remarkable-usb-api.md
- RCU features, licensing, and compatibility: https://www.davisr.me/projects/rcu/
- MIT-licensed bulk USB exporter: https://github.com/chopikus/rm-exporter
- Alternative Tauri/SSH reference (license must be verified): https://github.com/mahiatlinux/remarkable-webui
- Tauri prerequisites (Linux distros, macOS): https://v2.tauri.app/start/prerequisites/
- Tauri WebDriver testing (`tauri-driver`, Linux/Windows only): https://v2.tauri.app/develop/tests/webdriver/
- Tauri IPC mocking for frontend tests: https://v2.tauri.app/develop/tests/mocking/
- Tauri plugins, Linux packaging, and macOS signing: https://v2.tauri.app/plugin/ · https://v2.tauri.app/distribute/ · https://v2.tauri.app/distribute/sign/macos/
- `keyring` crate (Keychain / Secret Service): https://crates.io/crates/keyring
- Tauri GitHub Actions pipeline: https://v2.tauri.app/distribute/pipelines/github/
- GitHub Actions runner billing: https://docs.github.com/en/billing/managing-billing-for-your-products/managing-billing-for-github-actions/about-billing-for-github-actions
- Readwise Reader API for optional later integration: https://readwise.io/reader_api

**Unverified until tested against the owner's device and machines:**
- Device: the installed firmware, response schemas, the listing HTTP method, folder upload behavior, native archive availability, the exact PDF export endpoint, sleep behavior, and whether the web-interface setting persists across reboots.
- Host: the host netmask (the address was observed as `10.11.99.11` on Linux), macOS Local Network permission behavior for `10.11.99.1`, how to read macOS Internet Sharing state without privileges, and default NetworkManager handling on the owner's Linux distro.

The plan deliberately puts all of these in Phase 0 instead of assuming community research covers every firmware revision and host setup.
