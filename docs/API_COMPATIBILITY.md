# USB web interface: observed behavior

What `pb-probe` has measured on real hardware. Community documentation is a starting point; this file is the evidence.

## reMarkable 2, firmware 3.29.1.0 (Linux host, 2026-11-12)

Full report: [phase0/report-3.29.1.0-linux.md](phase0/report-3.29.1.0-linux.md). Fixtures: `tests/fixtures/3.29.1.0/`.

| Operation | Result |
|---|---|
| `GET /documents/` and `POST /documents/` | Both return 200 with the same JSON array. 3–43 ms. |
| `GET /documents/{id}` | 200, children of the folder. |
| `GET /download/{id}/pdf` | 200, valid PDF. About 1.5 s to first byte for a one-page notebook. |
| `GET /download/{id}/placeholder` | 200, byte-identical size to `/pdf`. Both paths work. |
| `GET /download/{id}/rmdoc` | 200, valid ZIP in about 170 ms. Contains `.metadata`, `.content`, `.pagedata`, and per-page `.rm` files. No thumbnails inside. |
| `GET /thumbnail/{id}` | **No reply within 10 s, and the web server then stopped answering every request** (TCP connections accepted, no response) for over ten minutes. Switching the USB web interface off and on restored it. Seen once, on a notebook. Do not call this endpoint. |
| `POST /upload` | Not tested yet. |

**Listing schema.** Every item has exactly these fields: `ID`, `Parent`, `Type`, `fileType`, `VissibleName`, `VisibleName`, `ModifiedClient`, `Bookmarked`, `CurrentPage`.

- The title is sent under both spellings, `VissibleName` and `VisibleName`.
- `Type` is `CollectionType` or `DocumentType`. Folders have no `fileType` value; documents have `notebook`, `pdf` or `epub`.
- `Parent` is `""` for the root, otherwise a UUID. All 72 IDs were valid UUIDs.
- `ModifiedClient` is RFC 3339 UTC with 1 to 3 fractional digits (trailing zeros are dropped), for example `…:47.18Z`.
- Nothing identifies the device, model or firmware, and there is no size or page count.
- Response header: `content-type: application/json; charset=ISO-8859-1`, chunked. Whether non-ASCII titles are really Latin-1 or UTF-8 is not yet checked.

**Host networking.**

- USB ID `04b3:4010`, manufacturer string `reMarkable`, Linux driver `cdc_ether`.
- The host address and netmask changed across a firmware update on the same day: `10.11.99.11/27` before, `10.11.99.2/29` on 3.29.1.0. Never hard-code the host address.
- With the web interface switched off, the tablet answers pings and port 80 refuses connections. Port 22 (SSH) is open either way.
- With the tablet unplugged, `10.11.99.1` routes to the default gateway.

**Still open:** upload behavior and headers, behavior while the tablet sleeps, whether the web-interface setting survives a reboot, whether every thumbnail request hangs the server, title encoding, export time for large documents.
