# Captured session (v1.0.808.0)

`common/game_builds/v1.0.808.0_debug_unmodified/AdClientLog.txt` is the log of the debug client in a session against the Massive test servers on 2009-04-13. It is the only recording of real protocol traffic available for this book.

The debug build links the same Massive library as the retail build (`Library Version: XDK 7978.3`, protocol revision 5, the same paths) with logging compiled in. Its log names every field it writes and reads, and it dumps each request in hex. Its IDA database (`rev3/tools/ida`) has partial symbols, which gave the names of the parsers and writers cited in the other chapters.

The session comes from a different build than the one this book describes. A statement that rests only on the capture is therefore marked as coming from v1.0.808.0. Where the retail code was checked as well, the chapters say so.

## Session

| Time (UTC) | Event |
|---|---|
| — | LocateService to `locate.madserver.net` (XLSP server `MASSIVE-1VS100`, 65.55.179.12, port 1000); the client clock was not yet set |
| 05:41:25 | locate response: zone, media and impression server `tsdev.madserver.net` port 1000, auto reporting time 10, statistic period 10, heartbeat period 0, options 0 |
| 05:41:25 | OpenSession; response: player id 420718, session id 7417817, 18 zone names |
| 05:41:42 | EnterZone `CommercialBreak1`; response: 6 IE records, 2 orders, 6 assets |
| 05:41:43 | all 6 assets loaded from the cache, no download |
| 05:41:45 to 05:56:19 | 88 ImpressionUpdates, one every 10 seconds, all without records |
| 05:51:26 | Statistics: cache look-ups 12, hits 6, mismatches 0 |

No Heartbeat, PlayerUpdate, CloseSession or DownloadBinary request occurs in the log.

## What the capture confirms

Request dumps decoded with the tag tables of this book, without any unknown field:

| Request | Bytes | Fields in wire order |
|---|---|---|
| LocateService `C9` | 67 | `3D` `3E` `1D` |
| OpenSession `CB` | 177 | `3D` `3E` `3C` `18` `1B` `44` `1D` `16` `E4` `3B` `1E` |
| EnterZone `CD` | 75 | `47` `2A` `2B` `3B` `1E` |
| ImpressionUpdate `D3` | 62 | `2B` `2A` `21` `29` `3B` `1E` |
| Statistics `DA` | 65 | `2B` `2A` `58` `59` `5C` `3B` `1E` |

- Envelope: version `05`, type, `u32 L` counting the trailer, `1E 00 14` and an HMAC-SHA1 over type through `1E`. The client logs the type as `Block ID` (201 = `C9`, 203 = `CB`, 205 = `CD`, 211 = `D3`, 218 = `DA`) and the response types are one higher (202, 204, 206, 219).
- Acknowledgements are 29 bytes.
- `3B` is Unix time in milliseconds.
- `massive-proto` decodes the EnterZone, ImpressionUpdate and Statistics dumps and re-encodes them byte for byte (test `captured_requests_decode_and_reencode`).
- Field names: `2A` Massive player id, `2B` Massive session id, the OpenSession fields, the EnterZone bandwidth fields, the statistics counters, the locate fields and all record fields. They are given in the chapters.

## Differences from the retail build

| Topic | v1.0.808.0 | Retail v2.0.1149 |
|---|---|---|
| HMAC key | the OpenSession field `44` value, 30 hex characters, per session; all 91 signed requests verify with it | the literal `"1"×32`, sent in field `44` (static analysis, confirmed with a retail console) |
| SKU version, AdClient version | `1.0`, `XDK 7978.3` | `edge`, `XDK Test QFE 1v1009328.3` |
| Player block length | constant 19 for 21 bytes of content | 21 |
| OpenSession `1C` | optional multiplayer GUID string | not written |
| EnterZone | no player blocks | player blocks after `2B` |
| Impression reports | `DF` and `E0` blocks with per-view fields (see [Records](records.md#impression-records-e6-c)) | `E6` records with the IE name only |

## Transport observations

- The HTTP request carries an empty `Host:` header, `Content-Type:application/massive` and `User-Agent:Adclient Massive Inc./XDK 7978.3`. The test server answered as `Apache-Coyote/1.1`.
- The client keeps the connection open across requests. Roughly every 30 seconds a receive failed (`Error while receiving. Still have original request data, will try resending the request`); the client then reconnected and sent the request again, which succeeded.

## Limits

- No asset was downloaded, so the asset URL resolution and the SHA-1 check are not covered.
- All impression batches were empty (the client logged `Invalid ScreenSize width: 0 height 0` for the image IEs), so the meaning of `21` and `29` stays open.
- No response bodies are dumped, only their field values.
