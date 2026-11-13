# pb-probe report

- Firmware (as entered): `3.29.1.0`
- Host OS: `linux`
- Interface: `enp0s20f0u5c2`, host address `10.11.99.2`, netmask `255.255.255.248`
- USB: `04b3:4010` Some("reMarkable") Some("2")

## Requests

| Request | Status | Time | Content-Type | Bytes / error |
|---|---|---|---|---|
| `GET /documents/` | 200 | 11 ms | application/json; charset=ISO-8859-1 | 5780 |
| `POST /documents/` | 200 | 9 ms | application/json; charset=ISO-8859-1 | 5780 |
| `GET /documents/00000000-0000-4000-8000-000000000001` | 200 | 44 ms | application/json; charset=ISO-8859-1 | 395 |
| `GET /documents/00000000-0000-4000-8000-000000000002` | 200 | 6 ms | application/json; charset=ISO-8859-1 | 3820 |
| `GET /documents/00000000-0000-4000-8000-000000000003` | 200 | 8 ms | application/json; charset=ISO-8859-1 | 5416 |
| `GET /documents/00000000-0000-4000-8000-000000000004` | 200 | 4 ms | application/json; charset=ISO-8859-1 | 1938 |
| `GET /documents/00000000-0000-4000-8000-000000000005` | 200 | 44 ms | application/json; charset=ISO-8859-1 | 1213 |
| `GET /documents/00000000-0000-4000-8000-000000000006` | 200 | 44 ms | application/json; charset=ISO-8859-1 | 1135 |
| `GET /documents/00000000-0000-4000-8000-000000000007` | 200 | 4 ms | application/json; charset=ISO-8859-1 | 1694 |
| `GET /documents/00000000-0000-4000-8000-000000000008` | 200 | 44 ms | application/json; charset=ISO-8859-1 | 1211 |
| `GET /documents/00000000-0000-4000-8000-000000000009` | 200 | 3 ms | application/json; charset=ISO-8859-1 | 1512 |
| `GET /documents/00000000-0000-4000-8000-00000000000a` | 200 | 4 ms | application/json; charset=ISO-8859-1 | 2237 |
| `GET /documents/00000000-0000-4000-8000-00000000000b` | 200 | 44 ms | application/json; charset=ISO-8859-1 | 384 |

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

| Path | Status | First byte | Total | Bytes | Signature | Content-Type | Content-Disposition present |
|---|---|---|---|---|---|---|---|
| `/download/00000000-0000-4000-8000-000000000039/pdf` | 200 | 1140 ms | 1149 ms | 29420 | PDF | application/pdf | true |
| `/download/00000000-0000-4000-8000-000000000039/placeholder` | 200 | 1138 ms | 1147 ms | 29420 | PDF | application/pdf | true |
| `/download/00000000-0000-4000-8000-000000000039/rmdoc` | 200 | 138 ms | 154 ms | 246074 | ZIP | application/zip | true |

## Upload

Skipped: no `--allow-upload` given.

## To check by hand

- [ ] Does the web interface stay reachable while the tablet sleeps?
- [ ] Is the USB web interface setting still on after a reboot?
- [ ] Was the uploaded test file placed in the root folder on the tablet?
