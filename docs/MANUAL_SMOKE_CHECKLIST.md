# Manual smoke checklist

macOS has no WebDriver for its webview, so these are checked by hand on every macOS build. They are also the hardware checks for Linux.

## Both systems, with the tablet

- [ ] First launch shows the welcome guide; no network prompt other than macOS Local Network.
- [ ] With the tablet unplugged the header says "Not connected".
- [ ] Plugging in and unlocking the tablet changes it to "Connected" within about 5 seconds.
- [ ] The library shows the same folders and documents as the tablet, with the right type for each.
- [ ] Opening a folder and using the path at the top to go back works.
- [ ] Search finds a document inside a subfolder; the type buttons filter.
- [ ] Unplug the cable: the header says "Not connected" and the library stays visible with a "Showing the library as it was…" note.
- [ ] Plug back in: it reconnects and refreshes by itself.
- [ ] Switch off the USB web interface on the tablet, press Refresh after reconnecting: the app explains which setting to switch on.
- [ ] The privacy button says "Isolation unverified" (never "verified").

## macOS only

- [ ] The Local Network prompt appears on first connection and names PaperBridge.
- [ ] Deny it: note exactly what the app shows. Record it in `PLATFORM_NOTES.md`.
- [ ] Allow it in System Settings → Privacy & Security → Local Network: the app connects.
- [ ] Download a newer build: note whether macOS asks again.
- [ ] During the whole session, `nettop` or Little Snitch shows PaperBridge connecting only to `10.11.99.1`.
