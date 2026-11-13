# Platform notes

## Linux

**Build prerequisites (Arch):** `webkit2gtk-4.1`, `base-devel`, Rust via `rustup`, Node with `pnpm`. Debian/Ubuntu packages are listed in `.github/workflows/build.yml`.

```
pnpm install
pnpm tauri dev          # run the app with live reload
pnpm tauri build        # release build (AppImage/.deb built on Arch are not portable; use CI's)
```

**USB networking.** The tablet is a USB Ethernet device (`04b3:4010`, driver `cdc_ether`). NetworkManager creates a wired connection for it automatically and the tablet hands the computer an address in `10.11.99.x`. The address and netmask vary by firmware.

PaperBridge only talks to an interface that is a reMarkable USB device with such an address, and binds its socket to that interface. If no such interface exists it sends nothing.

**Troubleshooting.**

| Symptom | Likely cause |
|---|---|
| "No tablet found" | Cable is charge-only, tablet is locked, or `cdc_ether` is missing (`lsmod \| grep cdc_ether`). |
| "USB web interface is switched off" | The setting under Menu → Settings → Storage is off. |
| "Tablet not responding" | The tablet's web server is stuck. Switch the setting off and on. |
| Privacy warning about the USB interface | The interface has no tablet address, typically because its NetworkManager profile is set to "Shared to other computers". Set it back to automatic (DHCP). |

**NVIDIA on Wayland.** WebKitGTK's DMA-BUF renderer crashes with `Error 71 (Protocol error)` on Wayland with the NVIDIA driver (seen on Hyprland). PaperBridge sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` for itself in that case. Set the variable yourself to override.

## macOS

There is no local toolchain requirement: builds come from the `macos` job in GitHub Actions (Actions tab → Build → Run workflow).

1. Download the `PaperBridge-macos` artifact and open the `.dmg`.
2. The build is ad-hoc signed, so clear the quarantine flag once per download:
   ```
   xattr -dr com.apple.quarantine /Applications/PaperBridge.app
   ```
3. On first connection macOS asks for Local Network access. Allow it.

**Known gaps on macOS (Phase 1):**

- The app accepts any interface with a `10.11.99.x` address; it does not yet confirm the interface is a USB device.
- Host privacy checks are not implemented; the badge says so.
- How a Local Network denial looks to the app is unconfirmed. It is expected to appear as "Blocked by macOS" and may instead appear as "USB web interface is switched off".
- Because each build has a new ad-hoc signature, macOS may ask for Local Network access again after every download.
