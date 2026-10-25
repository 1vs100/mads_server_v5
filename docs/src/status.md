# Reverse engineering status

Last reviewed: 2026-10-25.

This page is the single list of what is known, what is uncertain and what remains to be done. The chapters hold the details. The Rust implementation (`crates/massive-proto`) encodes the same state: every schema entry carries a layout and a meaning confidence, and the field server `massived` reports the observations listed under [Field verification](#field-verification).

## Known [C]

| Area | Statement | Chapter |
|---|---|---|
| Envelope | `05 | type | u32 L | fields | [1E 00 14 MAC]` in both directions; `L` counts the trailer | [Envelope](envelope.md) |
| Version byte | `05` on requests and responses; responses with another value are rejected | [Responses](responses.md) |
| MAC | HMAC-SHA1 over type through the `1E` tag; retail static analysis: key `"1"x32` literal, responses verified with the literal, not with field `44` | [Envelope](envelope.md) |
| Type bytes | eight requests (`C9 CB CD D1 D3 D6 D8 DA`); each response type is the request type + 1 | [Requests](requests.md), [Responses](responses.md) |
| Unknown tags | tags at or above `C8` carry a `u32` length and are skipped; unknown tags below `C8` fail every parser | [Body grammar](body-grammar.md) |
| Request bodies | all eight writers, including field order and the prepend/append order of the session pair | [Requests](requests.md) |
| Session pair | `2A` Massive player id, `2B` Massive session id | [Body grammar](body-grammar.md) |
| Zone announcement | OpenSession `47` names fill the client zone list; EnterZone for a name missing from a non-empty list fails locally with error 300 | [Responses](responses.md) |
| EnterZone bandwidth fields | `10` total size u32, `11` total time u32, `12` total items u16, written together and only when all are non-zero | [Requests](requests.md) |
| Field names | OpenSession `3C` `18` `1B`, statistics `58` `59` `5C`, locate tags, IE, order and asset fields, from the v1.0.808.0 parsers and writers | [Captured session](capture.md) |
| Wire format against real traffic | 92 request dumps of the v1.0.808.0 capture decode with the tag tables; `massive-proto` re-encodes three of them byte for byte | [Captured session](capture.md) |
| Device token | lowercase hex MD5 of `AA:BB:CC:DD:EE:FF`, 32 characters on the wire, no NUL | [Setup](setup.md) |
| Response parsers | the locate, OpenSession, EnterZone and five acknowledgement parsers, their tags and their exact field counts | [Responses](responses.md) |
| Locate response | all tags of `sub_824FEBE8`; exactly one `3B` required; no MAC | [Responses](responses.md) |
| Records | MAO, order and asset readers: tags, readers, counted fields, defaults; media type compare chain including the rejected boundary values | [Records](records.md) |
| Downloads | size check against asset field `07`, then plain SHA-1 against the first 20 bytes of field `04` | [Downloads](downloads.md) |
| HTTP request | template, `Content-Type:application/massive`, user agent, `GET` for downloads | [Transport](transport.md) |

## Uncertain [I]

| # | Item | Basis | Chapter |
|---|---|---|---|
| I2 | Retail binding of the statistic (`3F`) and auto reporting (`0D`) timers to their requests | names and v1.0.808.0 timing known; no retail Statistics or ImpressionUpdate seen yet | [Lifecycle](lifecycle.md) |
| I5 | Static path object classes for the media type ranges | analogy with the dynamic path | [Records](records.md) |
| I7 | Transport is plain HTTP with no TLS | no TLS code in the transport path; the v1.0.808.0 capture logs plain HTTP headers | [Transport](transport.md) |
| I8 | Mapping of Lua wrappers to library functions | indirect dispatch | [Lua integration](lua.md) |
| I9 | Client state enumeration (`+0x10`) | values seen, not named | [Lifecycle](lifecycle.md) |
| I10 | Subscriber and zone index layout; impression record object layout | partial reads | [Records](records.md) |

## Unknown [U]

| # | Item | Where to look |
|---|---|---|
| U1 | Meaning of impression counters `21`, `29` | `sub_82503520`, request `+0x50/+0x52` |
| U4 | Use of the locate server time `3B` (`+0x70`) and the bits of the server configurable options `3A` | locate completion handler `0x824F2E50` |
| U6 | Player id (16 bytes of player block field `41`) | client table `+0x90` writer |
| U7 | `E5`/`17` string, the full set of asset type `0B` values | `CMassiveAsset` users |
| U8 | Media type, rotation type and IE id values of the production server | test server values are in [Captured session](capture.md) |
| U9 | How the client resolves the URL path against the media server | URL parser behind `URL: %s` (`0x82082AF8`); the capture has no download |
| U10 | Message text for codes `0x258`, `0x25B` to `0x25E` | string table lookup in `sub_824F8C08` |
| U11 | Retry and timeout constants of `TransactionHTTP` | `0x825061F0`, `0x82506DD0` |
| U12 | Whether the client validates response timing | update loop `0x824F3410` |
| U13 | Manifest layout (`mani.fest`) | `massive_ManifestParse` |
| U14 | HTTP response handling: accepted status codes, header case, connection reuse | status parser strings at `0x82084C58`, `0x82084C80` |

## To do

Static analysis:

1. Trace the update loop (`0x824F3410`) to bind the intervals `54`, `3F` and `0D` to the heartbeat, statistics and impression requests (I2, U12).
3. Read the URL parser used by `CreateDownloadBinaryRequest` and `RequestDownloadBinary` (U9).
4. Read the HTTP status line and header parser of `CMassiveSocket` (U14).
5. Follow `+0x70`, `+0x78` and `+0x90` of the locate request into the completion handler (U4).
6. Map the error codes to message texts (U10).

### Field verification

Each item below is checked with `massived` against a console or emulator. The daemon logs the named event; see `crates/massived/README.md`.

| Statement | Event | Resolves |
|---|---|---|
| The client accepts the response framing (version `05`, MAC, counts) | the client proceeds to the next request: `request` events for OpenSession, then EnterZone | envelope and parsers |
| Which interval drives which request | `request` with `since_previous_ms` per type and session, compared with the configured `54` and `3F` | I2 |
| Meaning of the impression counters | `impression_update` with `records`, `value_21`, `value_29` | U1 |
| Asset URL form and download behavior | `asset_request` with the request target and `Host` header | U9 |
| Media type ranges and dynamic creator | `asset_request` per IE after EnterZone with test content for each range | I5 |
| HTTP behavior | `http_request` with raw headers; `connection_reused` | U14, I7 |
| Unknown fields | `unknown_field` and `decode_failed` with offset and hex | any |

## Resolved

| Date | Former item | Answer | Chapter |
|---|---|---|---|
| 2026-10-25 | Use of the OpenSession zone names | EnterZone accepts only announced zones (`sub_824F1728`), confirmed with a retail console | [Responses](responses.md) |
| 2026-10-25 | Retail HMAC key (I12) | retail console session against `massived`: field `44` is `"1"x32` and every request MAC verifies with the literal | [Envelope](envelope.md) |
| 2026-10-25 | Retail request time (I11) | Unix ms, synchronized to the locate server time (first request 8 ms after the server time sent) | [Envelope](envelope.md) |
| 2026-10-25 | Heartbeat timer (part of I2) | `54`: with 1 minute the retail client sent a Heartbeat every 60 s | [Lifecycle](lifecycle.md) |
| 2026-10-25 | Locate service names (I1) | v1.0.808.0 parser: zone, media and impression server | [Responses](responses.md) |
| 2026-10-25 | EnterZone `10`, `11`, `12` as peer address (I3) | bandwidth total size, time and items | [Requests](requests.md) |
| 2026-10-25 | Asset `05` as MAO `24` reference (I4) | the capture's IE asset arrays hold asset ids | [Records](records.md) |
| 2026-10-25 | Asset `15` as download flag (I6) | crex id | [Records](records.md) |
| 2026-10-25 | Statistics counters (U2) | cache look-ups, hits, mismatches | [Requests](requests.md) |
| 2026-10-25 | Request time `3B` (U3) | Unix ms in the v1.0.808.0 capture; retail moved to I11 | [Envelope](envelope.md) |
| 2026-10-25 | Locate value `56` (part of U4) | logging level | [Responses](responses.md) |
| 2026-10-25 | OpenSession `3C`, `18`, `1B` (U5) | session type, encryption algorithm, hash algorithm | [Requests](requests.md) |
| 2026-10-25 | Asset `06`, asset `02` encoding (part of U7) | rotation duration in ms; expiration in Unix ms | [Records](records.md) |
| 2026-10-25 | `2A` as session token | Massive player id | [Body grammar](body-grammar.md) |
| 2026-10-25 | CloseSession body `2A` only | `2A` and `2B` | [Requests](requests.md) |
| 2026-10-25 | ImpressionUpdate order `*` then `+` | `+` then `*` | [Requests](requests.md) |
| 2026-10-25 | Response status byte `00` | it is the version byte `05` | [Responses](responses.md) |
| 2026-10-25 | Locate reply tags (former item 7) | decoded from `sub_824FEBE8` | [Responses](responses.md) |
| 2026-10-25 | Blob encoding of EnterZone `10`, `11` (former item 3) | `u32`; `12` is `u16`; all or none | [Requests](requests.md) |
| 2026-10-25 | Asset tags `02` / `0A` assignment, `13` vs `15` (former item 4) | jump table resolved: `02` u64, `04` digest, `0C` url, `15` counted flag; `00`, `01`, `03` rejected | [Records](records.md) |
| 2026-10-25 | Asset and order counters (former item 6) | eight counted asset fields, two counted order fields, exact match required | [Records](records.md) |
| 2026-10-25 | "Trailing mode value 3" in the EnterZone parser | the count of `1E`, `25`, `55` | [Responses](responses.md) |
| 2026-10-25 | Media type chain `0x400`/`0x800`/`0x1000` | ranges; the boundary values themselves are rejected | [Records](records.md) |
| 2026-10-25 | Device token length 33 | 32 characters on the wire | [Setup](setup.md) |
| earlier | Origin of OpenSession strings `a`, `b` | SKU name and version from `Initialize` | [Setup](setup.md) |
| earlier | Origin of the device token | MD5 hex of the console MAC string | [Setup](setup.md) |
| earlier | Expected digest for non-download responses | the response's own `1E` field | [Envelope](envelope.md) |
| earlier | Response HMAC coverage | everything except the version byte and the trailer | [Envelope](envelope.md) |
| earlier | Downloads use HMAC | they use plain SHA-1 | [Downloads](downloads.md) |
| earlier | MAO `0A` is a kind byte | it is a `u16` size threshold; `24` is an asset-id array | [Records](records.md) |
| earlier | Heartbeat and statistics bodies | decoded | [Requests](requests.md) |
| earlier | Impression record layout | `E6` record with the IE name in this build | [Records](records.md) |

## Reproduction

The helper functions (`dis`, `callers`, `strrefs`, `ccalls`) are in `work/ida/helpers.py`. The renames are in `work/ida/renames.py`. The Rust implementation and its tests are in `crates/massive-proto`; the Python reference is in `work/massive_envelope.py`.
