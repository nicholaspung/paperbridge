# pb-probe report

- Firmware (as entered): `3.29.1.0`
- Host OS: `macos`
- Interface: `en7`, host address `10.11.99.2`, netmask `255.255.255.248`
- USB: not verified on this OS

## Requests

| Request | Status | Time | Content-Type | Bytes / error |
|---|---|---|---|---|
| `GET /documents/` | 200 | 15 ms | application/json; charset=ISO-8859-1 | 5780 |
| `POST /documents/` | 200 | 11 ms | application/json; charset=ISO-8859-1 | 5780 |
| `GET /documents/00000000-0000-4000-8000-000000000001` | 200 | 2 ms | application/json; charset=ISO-8859-1 | 395 |
| `GET /documents/00000000-0000-4000-8000-000000000002` | 200 | 4 ms | application/json; charset=ISO-8859-1 | 3820 |
| `GET /documents/00000000-0000-4000-8000-000000000003` | 200 | 6 ms | application/json; charset=ISO-8859-1 | 5416 |
| `GET /documents/00000000-0000-4000-8000-000000000004` | 200 | 4 ms | application/json; charset=ISO-8859-1 | 1938 |
| `GET /documents/00000000-0000-4000-8000-000000000005` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 1213 |
| `GET /documents/00000000-0000-4000-8000-000000000006` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 1135 |
| `GET /documents/00000000-0000-4000-8000-000000000007` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 1694 |
| `GET /documents/00000000-0000-4000-8000-000000000008` | 200 | 2 ms | application/json; charset=ISO-8859-1 | 1211 |
| `GET /documents/00000000-0000-4000-8000-000000000009` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 1512 |
| `GET /documents/00000000-0000-4000-8000-00000000000a` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 2237 |
| `GET /documents/00000000-0000-4000-8000-00000000000b` | 200 | 1 ms | application/json; charset=ISO-8859-1 | 384 |
| `GET /thumbnail/00000000-0000-4000-8000-00000000000c` | error | | | timed out after 10s while waiting for a short device response |

## Root listing response headers

- `content-type: application/json; charset=ISO-8859-1`
- `transfer-encoding: chunked`

Listing method used for the walk: `GET`

## Library shape

- Items: 72
- Folders walked: 11
- Items with an ID that is not a UUID: 0
- `Type/fileType` = `CollectionType/`: 11
- `Type/fileType` = `DocumentType/epub`: 8
- `Type/fileType` = `DocumentType/notebook`: 46
- `Type/fileType` = `DocumentType/pdf`: 7

## Fields seen

| Field | JSON types | Example (safe fields only) |
|---|---|---|
| `Bookmarked` | bool | `false` |
| `CurrentPage` | number | `8` |
| `ID` | string | UUID |
| `ModifiedClient` | string | (timestamp, 23 chars) |
| `Parent` | string | `""` |
| `Type` | string | `DocumentType` |
| `VisibleName` | string | (text) |
| `VissibleName` | string | (text) |
| `fileType` | string | `notebook` |

## Exports

Skipped: no `--export-id` given.

## Upload

Skipped: no `--allow-upload` given.

## To check by hand

- [ ] Does the web interface stay reachable while the tablet sleeps?
- [ ] Is the USB web interface setting still on after a reboot?
- [ ] Was the uploaded test file placed in the root folder on the tablet?
