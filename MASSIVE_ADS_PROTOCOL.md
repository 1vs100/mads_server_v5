> **Superseded.** The complete protocol description is now the mdbook in `massive-book/` (build with `mdbook build massive-book`). This file is kept as the first-pass notes.

# MASSIVE ADS protocol — 1 vs 100 (Xbox 360, retail v2.0.1149.0.02, title 584A07D1)

Reverse-engineering notes: Lua game layer + native MassiveAdClient3 library.

Sources:
- Lua: `common/game_builds/v2.0.1149.0.02-2009-10-26_retail/584A07D1/00000002/extracted/lua/` (`ads.lua`, `epads.lua`, `adscreens.lua`, `config.lua`, `data.lua`, `onevslive.lua`, `stateloadingscreen.lua`, `statelobby.lua`, `stateroundstart.lua`, `stateextendedplay.lua`).
- Native: `default.xex` loaded in the IDA driver `rev3/tools/ida-1vs100-retail` (PPC, base 0x82000000).
- Work products in this folder: `work/asm/` and `work/asm2/` (disassembly listings), `work/INDEX.md` (function index), `work/massive_envelope.py` (reference framing code + self-test), `work/strings_massive_region.txt` (strings).

## 0. Confidence legend

- **[C] confirmed**: read directly from the disassembly or Lua source.
- **[I] inferred**: strongly suggested by code, not fully traced.
- **[U] unknown**: identified but not decoded.

No live capture exists and the server is not reachable from this environment. Every wire-format statement is from static analysis. Treat the framing as [C] for the code paths read, but validate it against a capture before relying on it.

---

## 1. Architecture

```
  Lua game code (ads.lua & friends)          g_Massive = Massive()   (onevslive.lua:613)
            │  g_Massive:Initialize / EnterZone / BindIE / ReportImpression / ...
            ▼
  Native Lua binding "Massive" class         method table @ 0x8200CDA0 (14 entries) [C]
            │  wrapper per method (0x8218xxxx–0x8219xxxx)
            ▼
  MassiveAdClient3 library (XMassiveAdClient) 0x824F0000–0x82510000 (≈322 functions) [C]
     ├─ CRequestManager      queue + SubmitRequest                        (0x824FA7D8/0x824FAC58)
     ├─ CRequestObject       one per request type; vtable +4 = response parser
     ├─ CNetworkManager      server address/port, DNS "locate" pair       (0x824FADC0..0x824FB438)
     ├─ TransactionHTTP      HTTP/1.1 request builder + socket           (0x825061F0/0x82506308)
     ├─ CMassiveZoneManager  zones → MAO lists → subscribers → assets    (0x824F41F0…)
     ├─ CMassiveAsset        downloaded binary/video/texture             (0x824F7BD8, 0x824F8208)
     └─ MassiveThread        worker loop                                 (0x82501430)
  Massive servers:  locate.madserver.net  →  /adsrv/5/*  and  /impsrv/5/*   [C]
```

Thread model: `MassiveThread` (0x82501430) performs network I/O. Lua calls such as `EnterZone` only queue a request, and the state is observed later through `IsInZone`, `IsConnected`, and `GetState`.

---

## 2. Lua-facing API

### 2.1 Native method table [C]

The method table is at `0x8200CDA0`. Each 8-byte entry is `{name_ptr, cfunc_ptr}`.

| Lua method | Native wrapper | Used by shipped Lua | Notes |
|---|---|---|---|
| `EnterZone(zoneName)` | 0x82190728 | yes (ads.lua:505) | queues the zone-enter request |
| `ExitZone()` | 0x82190820 | yes (ads.lua:494) | |
| `BindIE(ieName, element [, bool])` | 0x82191160 | yes (ads.lua:510–527, 1036) | binds an inventory element (IE) to an XUI video/image element. The optional `true` is used by disk-sourced videos (ads.lua:1036). |
| `UnBindIE(ieName)` | 0x82190230 | yes (ads.lua:539–545, 1049) | |
| `UseNetwork(…)` | 0x8218CC78 | **no** | present in table only |
| `SetNetBandwidth(kbpsIn, kbpsOut)` | 0x8218CCD0 | yes (ads.lua:72) | values 7 KB/s PT and EP (data.lua:739–740) |
| `ReportImpression(ieName, count)` | 0x8218C978 | yes (ads.lua:1120, 1545, 1647) | count is always `1` |
| `IsConnected()` | 0x8218CA18 | yes | |
| `GetResults()` | 0x8218CA68 | yes (ads.lua:49) | returns `(ok, errorCode)` after `Initialize` |
| `Initialize(sku, version)` | 0x8218D308 | yes (ads.lua:47) | `"mgs_1_vs_100_x360"`, `"edge"` (data.lua:731–732) |
| `Shutdown()` | 0x8218D3A8 | **no** | present in table only |
| `GetState()` | 0x8218CAD0 | yes (ads.lua:1841) | returns `{stateCode, stateStr, errorCode, errorStr}` |
| `IsInZone()` | 0x8218CBB0 | yes | |
| `GetCurrentZoneName()` | 0x8218CBF8 | yes (ads.lua:693, 1845) | |

The wrappers all call a common object-fetch helper (`sub_8218C8D0`). The final dispatch from wrapper into the library object is an **indirect** call. A static call-graph walk did not reach `0x824Fxxxx` through these wrappers, so the mapping from each Lua method to a specific library function is [I] at best. The library-side functions in §4–§6 are identified from their debug strings and call structure.

### 2.2 Lua call sequence (game side)

1. **Boot**: `onevslive.lua:613` creates `g_Massive = Massive()`.
2. **Init task** (`InitializeMassive`, ads.lua:44): `Initialize("mgs_1_vs_100_x360", "edge")`, then `Sync()`, then `GetResults()`.
   - On failure: log `LuaErrors` report `MASSIVE_INIT` (fatal = "No"), wait `random(minInitMassiveWait=1, maxInitMassiveWait=100)` seconds, retry as a global task.
   - On success: `SetNetBandwidth(kbps, kbps)` with kbps = `massivePTKBPS` (7) or `massiveEPKBPS` (7), overridable by server config (config.lua:353–392). `DEFAULT_MASSIVE_BW_OVERRIDE` forces 0.
3. **Staggered start** (`InitMassiveAndEnterLobbyZone`, ads.lua:91): spreads init and zone-enter over the time left before the game's first sponsor ad (`roleCutoffTime` + program start). Random waits use a 1/8 s granularity. JIP players shift their start to `now + 30 s`. The initial zone is `introVideoAdZoneIndex = 1`, which is `Lobby_PT` (PT) or chosen via `PredictQuestions` (EP).
4. **Zone entry** (`WaitToEnterNextAdZone_Task`, ads.lua:439–537):
   - sets `currentZone` / `currentZoneName`
   - waits `waitLength` (`random(minEnterZoneWait=1, maxEnterZoneWait=100)` unless `DEFAULT_ENTERZONE_IMMEDIATE`)
   - waits for any playing Massive ad to finish
   - if not connected, runs `InitializeMassive` now
   - `ReleaseMassiveIE()`, then `ExitZone()` if already in a zone
   - `EnterZone(zoneName)`
   - `BindIE` for the 4 static images (`Text_1..Text_4`) and for the video IEs (`Video15_1..6`, `Video30_1..3`). Binding is what starts the downloads.
5. **Ad break**: `InsertAdBreak` (ads.lua:569) → `LoadFirstAd` → `PlaySetAds` (videos from IE) → `ShowStaticAds` (images) → `PlayHouseAds` / `PlaySponsorAds` / `PlayDiskVideos` as fallbacks.
6. **Impressions**: `EndPlayVideoAd` (ads.lua:1104) and static-image loop (ads.lua:1545/1647) call `ReportImpression(name, 1)`, unless the ad is a house ad, the caller passed `bDontReportImpression`, or `GameVars.Ads.obstructionDuringAd` is set (the impression is then skipped).
7. **Stats** (`UpdateAdBreakStats`, ads.lua:1839): reports `"AdBreakStats"` or `"LobbyFPP"` through `LuaErrors` containing `GetState()`, `IsConnected()`, `GetCurrentZoneName()`, the download time since `zoneEnterTime`, and a per-slot download bitmap (`T.1:1 … V30.1:0 … V15.6:1`).

### 2.3 Zones, IEs, and media (data.lua) [C]

| Item | Values |
|---|---|
| PT zones (`ZonesPT`) | `Lobby_PT`, `CommercialBreak1` … `CommercialBreak8` (9 zones; index 1 = lobby) |
| EP zones (`ZonesEP`) | `Lobby_EP`, `CommercialBreakEP_1` … `CommercialBreakEP_4` |
| Static IEs | `Text_1` … `Text_4` (`MassiveAdElements[1..4]`) |
| 15-second video IEs | `Video15_1` … `Video15_6` |
| 30-second video IEs | `Video30_1` … `Video30_3` |
| Lobby intro | `introVideoAdStartTime = 5`, `introVideoAdTime = 30`, zone index 1 |
| Ad timing | `AdTimingReset[]` (17 entries): `startTime` (minutes), `adTime`, `liveHostPriorTime`, `liveHostPostTime`, `outroStaticAd`, `zoneIndex` |
| Local files | `ShowDrive() .. FileDirs.gAdsDir .. name .. ".wmv"` for UserEd, Promo, HouseAd15/30, SeasonSponsor15/30 |

Fallback rules (ads.lua:~252–262): for country `ca`, `de`, `fr`, or when no sponsor files exist, sponsor lists are replaced by UserEd / Promo files, and `DontReportSeasonSponsorImpression = true`.

### 2.4 Retail vs debug [C]

`luamockserver.lua` has test overrides (e.g. `massivePTKBPS = 11`, `maxInitMassiveWait = 300`). `IsNotRetailBuild()` gates debug IE-name labels only. Neither changes the wire protocol.

---

## 3. Transport [C]

### 3.1 Endpoints

| Purpose | Path | Type code | Body sealed? | Source |
|---|---|---|---|---|
| Locate the session server | (DNS) `locate.madserver.net`, then `/adsrv/5/locateService` | 0xC9 | no (no time, no sig) | 0x824FE9B0, 0x824FEAF8 |
| Open session | `/adsrv/5/openSession` | 0xCB | yes (time + sig) | 0x82501608, 0x82501818 |
| Close session | `/adsrv/5/closeSession` | 0xD1 | yes (time + sig) | 0x824FE658, 0x824FE7F0 |
| Update players | `/adsrv/5/updatePlayers` | 0xD8 | yes | 0x824FF178, 0x824FF310 |
| Enter zone | `/adsrv/5/enterZone` | 0xCD | yes | 0x82503718, 0x82503798 |
| Impression report | `/impsrv/5/activity` | 0xD3 | yes | 0x82502BD8, 0x82503520 |
| Heartbeat | `/impsrv/5/heartbeat` | — (body writer not traced) | [U] | 0x82507348 |
| Statistics | `/impsrv/5/stats` | — | [U] | 0x82507668 |
| Asset download | URL from asset record | 0x54 (GET) | no envelope; asset HMAC check | 0x82505420, 0x824F7E90 |

### 3.2 HTTP framing (exact template) [C]

Built by `sub_82506308` (TransactionHTTP). Fields are substituted by `sub_824B0720`:

```
POST <path> HTTP/1.1\r\n
Host:<host>:<port>\r\n
Content-Length:<len(body)>\r\n
Content-Type:application/massive\r\n
User-Agent:Adclient Massive Inc./XDK Test QFE 1v1009328.3\r\n
\r\n
<sealed body>
```

- `POST` is used for every request type except the asset download (`GET`, selected when request type == 0x54).
- The User-Agent suffix `XDK Test QFE 1v1009328.3` is a hard-coded test build string. It also appears as the body build field (tag 0x16, §5.2).
- No TLS code was observed in the transport. Socket I/O is `CMassiveSocket` (0x82506DD0). The transport appears to be plain HTTP over TCP, but the port comes from `CNetworkManager::SetServerPort` and was not traced to a constant. Treat as [I].

### 3.3 Locate and address resolution [C/I]

- `CNetworkManager` stores a server address (`SetServerAddress`), a port (`SetServerPort`), and a host name (`GetServerHostName`).
- A DNS thread ("Massive DNS", 0x824FB140) resolves `locate.madserver.net` (string at 0x820831D8).
- `RequestLocateService` returns a list of address/port pairs (`CAddressIndexPair`, `CPortIndexPair`). The client picks one and then opens a session. The exact pair layout was not decoded [U].

---

## 4. Session and request lifecycle

Derived from `XMassiveAdClient::Initialize/Update/EnterZone/ExitZone/ShutdownBegin` and the `Create*Request` call structure. Each step is [C]; the sequencing between steps is [I]. Exact retry and timeout constants are [U].

```
Initialize(sku, ver)   → validates SKU / version / public key, creates client, starts MassiveThread
   ├─ CNetworkManager: DNS "locate.madserver.net" → LocateService (0xC9, plain)
   ├─ OpenSession (0xCB)   → response: session token (*), session id (+), zone list (G records)
   ├─ UpdatePlayers (0xD8) → periodic player block updates (4 slots max)
   ├─ EnterZone (0xCD)     → response: MAO list (0xDD), orders (0xE3) with assets (0xDE)
   │       └─ DownloadBinary (GET) per asset; verify 20-byte validity hash; CMassiveAsset::HandleResponse
   ├─ ImpressionUpdate (0xD3) at /impsrv/5/activity (batched; FlushImpressions)
   ├─ Heartbeat /impsrv/5/heartbeat, Stats /impsrv/5/stats (periodic)
   └─ CloseSession (0xD1)  → on ExitZone / Shutdown
```

Notes:
- `CreateOpenSessionRequest` first queues `CreateCloseSessionRequest` if `qword_82A0EEA8 != 0` (session already open). If it is 0 it logs `No Session to Close`.
- The `GetState()` values come from `XMassiveAdClient` state/error tables (strings `STATE_ENTERZONE_REQUEST`, `STATE_ENTERZONE`, `EnterZone: (%s) has error = %d !`).
- Request status is tracked per object (`REQUEST_STATUS_COMPLETE_PARSED`, and others); the request lifecycle is `Create → Submit (SubmitRequest, 0x824FAC58) → transport → Parse`.

### 4.1 Client error and status messages [C]

Strings from the initialisation and runtime paths, in the order they appear:
`Invalid SKU`, `Invalid SKU version`, `Invalid public key`, `Invalid initialization structure`, `AdClient is already initialized`, `AdClient is not initialized`, `Could not contact Massive servers`, `Failed to Locate Service: %d`, `Open Session Failed: %d`, `Zone Does Not Exist`, `Zone Name Does not Exist on the Server: %s`, `Cannot Find Zone: %s`, `No zone has been entered`, `Could not close the current session…`, `Could not create a new session`, `Cannot join multiple sessions`, `Invalid session ID`, `Trying to update before internal time has been reached`, `Attempting to flush impressions before internal time has been reached`, `Massive Headers do not match the Version of the Massive library`, `The shutdown process has not completed in an acceptable amount of time`, `A failure in the shutdown process has occurred`.

Network-level codes: `Network bad status`, `Network disconnect error`, `Network connect error`, `Network receive error`, `Network send error`, `Error contacting domain name server`, `Could not resolve the network address`, `Network socket error`.

Asset-level codes: `Invalid asset data length`, `Invalid asset date`, `Invalid asset size`, `Asset data failed validity hash`, `URL is not properly formatted or inaccessible`, `Failed to download asset`, `Invalid asset ID`, `Asset already exists`.

Lua-side error record names: `MASSIVE_INIT` (ads.lua:53), `AdBreakStats`, `LobbyFPP` (ads.lua:1839–1860).

---

## 5. Message envelope and body grammar

### 5.1 Sealing algorithm (sub_824FA478) [C]

The body is built in an append buffer first. Then `sub_824FA478(this, type, timeFlag, signFlag)` seals it:

1. If `timeFlag`: append `';'` (0x3B) followed by an 8-byte 64-bit BE value from `sub_824FE140` (the time source; its exact meaning is [I], probably a session-relative clock).
2. If `signFlag`: append `0x1E`.
3. Prepend `u32 BE L`, where `L = len(buffer) + 22` if signed, otherwise `len(buffer)`. The 22 bytes are the trailer `00 14` + 20-byte MAC.
4. Prepend the `type` byte.
5. If `signFlag`: `MAC = HMAC-SHA1(key="1"×32, data=type|L|body|…|1E)` (`sub_82505760`), appended as `00 14` + MAC.
6. Prepend `0x05`.

Final layout:

```
05 | type | u32BE L | body… | [3B | time64] | [1E] | [00 14 | HMAC-SHA1(20)]
   └────── HMAC covers: type | L | body | [3B time] | [1E] ──────┘
```

Observed type codes (first argument to `sub_824FA478`): `0xC9` locate (no time, no sig), `0xCB` open session, `0xCD` enter zone, `0xD1` close session (body starts with a prepended `2A`), `0xD3` impression, `0xD8` update players.

Notes:
- The HMAC key is the literal 32-character string `11111111111111111111111111111111` (ASCII 0x31 ×32). It is a constant, not a secret. It is also sent as field `D` in the OpenSession body (§5.2).
- The reference implementation `work/massive_envelope.py` reproduces `seal()`/`unseal()`. Its self-test checks the round trip and the MAC. It has **not** been checked against a real server.

### 5.2 Primitives (append semantics) [C]

| Writer | Encoding |
|---|---|
| tag byte | `1 byte` |
| `u16` (sub_824F9EE8) | `u16 BE` |
| `u32` | `u32 BE` |
| `u64` (sub_824F9F68) | `u64 BE` |
| string (sub_824FA640) | `u16 BE length` + bytes (max 2000) |

Some writers prepend instead of append. `sub_824FF310` (PlayerUpdate) uses prepends, so its fields end up in the reverse order given in §5.4.

### 5.3 Player block (sub_824FA218 WritePlayerUpdateBlocks) [C]

Up to 4 slots, read from the client object at `+0x90` (20-byte entries: `u32 flag`, then 16 bytes of player ID). The loop stops at the first empty slot. Each slot writes:

```
E4 | 00 00 00 15 | 57('W') | <slot index u8> | 41('A') | 00 10 | <16-byte player id>
```

`L = 0x15 = 21` covers the 21 bytes from `'W'` through the end of the ID.

### 5.4 Request bodies [C unless marked]

**OpenSession (0xCB, sub_82501818)**, appended in order:

```
3D '=' str16(a)            a = caller string (name not traced)
3E '>' str16(b)            b = caller string
3C '<' 01
18     00
1B     01
44 'D' str16("1"×32)       constant key string
[1D    str16(token33)]    token33 from sub_824F3A90 (per-console 0x21-byte value) — when flag set
16     str16("XDK Test QFE 1v1009328.3")
(player blocks §5.3)
```
Sealed with time + sig.

**PlayerUpdate (0xD8, sub_824FF310)**, the final body order (prepends reverse the write order):

```
2A '*' u64 qword_82A0EEB0   (session token, inferred)
2B '+' u64 qword_82A0EEA8   (session id, inferred)
(player blocks §5.3)
```
Sealed with time + sig.

**EnterZone (0xCD, sub_82503798)**:

```
47 'G' str16(zoneName)      zone name, e.g. "Lobby_PT"
2A '*' u64 qword_82A0EEB0
2B '+' u64 qword_82A0EEA8
(player blocks §5.3)
[10 blob  (sub_824F9F28)]   optional
[11 blob  (sub_824F9F28)]   optional
[12 u16   (sub_824F9EE8)]   optional count
```
Sealed with time + sig.

**Impression (0xD3, sub_82503520)**:

```
21 '!' u16(this+0x50)
29 ')' u16(this+0x52)
```
Sealed with time + sig. The per-impression records are added by `sub_82502C50` (not traced in detail) [U].

**LocateService (0xC9, sub_824FEAF8)**: unsealed, no time, no sig.

```
3D '=' str16(a)
3E '>' str16(b)
3C '<'
1D     str16(token33)     (only when the device-id source dword_82A0EF08 is non-null)
```

**CloseSession (0xD1, sub_824FE7F0)**: `2A '*'` is prepended, then sealed with time + sig. The rest of the body was not traced [U].

**DownloadBinary (type 0x54)**: HTTP `GET` to the asset URL stored in the request object (`+0x50`). No body.

**Heartbeat, Stats**: bodies not traced [U].

### 5.5 Field semantics of the globals [I]

| Global | Meaning | Evidence |
|---|---|---|
| `qword_82A0EEA8` | session id | written by OpenSession parse (`+` tag → this+0x58); used as "session open" test in CreateOpenSessionRequest |
| `qword_82A0EEB0` | session token / server token | written by OpenSession parse (`*` tag → this+0x50) |
| `dword_82A0EE0C` | client object pointer (`XMassiveAdClient`) | used by all request creators |
| `dword_82A0EF08` | console/device identity source | gates the `1D` token in OpenSession and LocateService |
| `dword_82A0EEBC` | singleton flag for zone state | — |

---

## 6. Response grammar

### 6.1 Common framing [C/I]

- The body is parsed from the buffer front. The first byte is consumed by `sub_824FA0B8`, which returns true only if that byte is `0x00`. Treat `0x00` as OK. A non-zero first byte causes a parse failure [I, from control flow].
- Then the parser expects `0xCC` (OpenSession, `sub_825016D8`) or `0xCE` (`Parse`, `sub_82504728`), followed by `u32 BE length`, followed by a sequence of tagged records until the length is consumed.
- At the end the parser verifies `HMAC-SHA1(key="1"×32, buffer)` against a 20-byte expected digest stored in the request object at `+0x30` (`sub_824F9300`). For downloads that digest is the asset validity hash. For other requests the source of the expected digest was not traced [U].

### 6.2 OpenSession response tags (sub_825016D8) [C]

| Tag | Action |
|---|---|
| `1E` | 20-byte signature marker (`sub_824FA108`) |
| `2A '*'` | `u64` → request `+0x50` (session token) |
| `2B '+'` | `u64` → request `+0x58` (session id) |
| `47 'G'` | zone/entry record (`sub_824FA168`, registered with `sub_824F1798`) |
| `CC` | section start |

### 6.3 Enter-zone / update response (sub_82504728 Parse) [C]

| Tag | Action |
|---|---|
| `1E` | signature token |
| `25 '%'` | `u16` → `+0x5C` (count A) |
| `55 'U'` | `u16` → `+0x6C` (count B) |
| `DD` | MAO / dynamic-creator record (`sub_82503928`) |
| `E3` | order block → `ReadOrderBlock` (`sub_82504510`) |
| other | skipped (`sub_824F96C0`) |

At the end, counts A and B must match the number of records parsed. A trailing mode byte must equal 3 [I]. The HMAC is then checked.

### 6.4 Order block (ReadOrderBlock, 0xE3) [C]

| Sub-tag | Action |
|---|---|
| `01`, `03`, `13` | `u32` read (order fields) |
| `DE` | asset record → `ReadAssetBlock` |

Creates a `CMassiveOrder` (0x24 bytes, `sub_8250B740`). On error: `CMassiveListNode for asset in ReadOrderBlock()`, `CMassiveListNode for order in Parse()`.

### 6.5 Asset record (ReadAssetBlock) [C/I]

Dispatch is by a jump table (`byte_82084998`) for sub-tags `0x00–0x0A`. Other sub-tags:

| Sub-tag | Action |
|---|---|
| `15` | `u32` → asset id |
| `E5` | nested block (`sub_82503DD8`) |
| `1E` | string (`sub_824FA168`) |
| `0x0B–0x14`, others | `u32`, `u16`, `f32` reads (`sub_824F94D0` for floats: screen size / angle), `sub_824F9608` (string + length) |

Assets are created by `sub_824F7BD8(asset_id, url, width, height, ...)`. The fields are: id, URL (from a length-prefixed string), size, date, validity hash (20 bytes), and dimensions. The exact per-field tag mapping for `0x00–0x0A` was not completed [U]. The error strings in §4.1 describe the validation performed on those fields.

### 6.6 Downloads and validity [C]

`CMassiveAsset::HandleResponse` (0x824F8208) receives the GET body. The check is HMAC-SHA1 over the downloaded bytes with key `"1"×32`, compared to the asset's validity hash. On mismatch, `Asset data failed validity hash`.

---

## 7. Impressions and reporting [C/I]

- Impressions are recorded per IE/MAO subscriber. `CRequestImpressionUpdate` (type 0x1065) is created from `CRequestManager` and sent to `/impsrv/5/activity`.
- `XMassiveAdClient::FlushImpressions` (0x824F1A28) forces a flush. It refuses if called before the internal time elapses (`Attempting to flush impressions before internal time has been reached`).
- Lua decides what counts as an impression (see §2.2 item 6). The native side batches them.
- `CMassiveRecordImpression` (0x82505EC0) and `CMassiveRecord` (0x82505FA8) are the per-record objects.
- Impression bodies contain the two `u16` counters from §5.4 (fields at +0x50/+0x52). Their meaning was not traced [U].

---

## 8. Local files and logs [C]

| Path (game-relative) | Use |
|---|---|
| `game:\ClientOverrides.ini` | client overrides read by the library |
| `game:\AdClient.csv` | client status CSV |
| `game:\AdClientLog.txt` | client log |
| `game:\AdClientMemoryUsage.csv` | memory usage log |
| `game:\Massive` | asset cache directory (`massive-cache-0`) |
| `%s\%s_%lu.TMP` | cache temp file pattern |
| `mani.fest` | asset manifest (`ManifestParse`, 0x82500960 / 0x82500BB8, not decoded [U]) |

Cache jobs: `CFileAsyncWorker` (`CCacheJobReadFile/WriteFile/DeleteFile/ClearCache/CreateCache`).

---

## 9. Constants and identifiers (quick reference) [C]

| Name | Value |
|---|---|
| SKU | `mgs_1_vs_100_x360` |
| SKU version | `edge` |
| HMAC key | `"1"` × 32 |
| Content-Type | `application/massive` |
| User-Agent | `Adclient Massive Inc./XDK Test QFE 1v1009328.3` |
| Build field (tag 0x16) | `XDK Test QFE 1v1009328.3` |
| Locate host | `locate.madserver.net` |
| API base | `/adsrv/5/…` (session), `/impsrv/5/…` (impressions/heartbeat/stats) |
| Max player slots | 4 |
| Max string | 2000 bytes |
| Asset validity digest | 20 bytes (HMAC-SHA1) |

---

## 10. Open items / not traced

1. Exact server address and port, and the locate response layout (`CAddressIndexPair`, `CPortIndexPair`).
2. Origin of the caller strings `a` / `b` in OpenSession and LocateService, and of the 0x21-byte `token33`.
3. Full decoding of the asset sub-tags `0x00–0x0A` and the order sub-tags beyond `01/03/13/DE`.
4. Heartbeat and stats body formats.
5. Impression record layout (`sub_82502C50`).
6. `mani.fest` format.
7. Source of the expected 20-byte digest in non-download responses (`this+0x30`).
8. Whether the response HMAC covers the whole buffer, including the `1E` marker. The code verifies the full buffer, so this needs a capture to confirm.
9. Mapping of each Lua wrapper to its exact library function. The wrappers dispatch indirectly.
10. Native retry and timeout constants.

---

## 11. Security observations [C/I]

- The HMAC key is the constant `"1"×32` and is embedded in every client. Any party can forge the envelope MAC. The MAC gives integrity against accidental corruption, not authenticity.
- The asset validity hash uses the same constant key. An attacker who controls the asset source can produce matching hashes.
- The build string `XDK Test QFE 1v1009328.3` is a test-build identifier in production traffic.
- The transport appears to be plaintext HTTP. The locate step is a DNS lookup of a fixed host name.
- The Lua layer reads `DEFAULT_MASSIVE_BW_OVERRIDE` and `DEFAULT_ENTERZONE_IMMEDIATE` (ads.lua:70, 423), but neither is defined anywhere in the shipped Lua. They are nil in retail, so the bandwidth override and immediate zone entry are inactive.

---

## 12. Reproducing the analysis

- Listings: `work/asm/*.s` (60 annotated key functions) and `work/asm2/*.s` (322 functions, one per file; `work/asm2/_index.txt` lists them).
- Function index with roles: `work/INDEX.md`.
- Framing self-test: `python3 work/massive_envelope.py`.
- The IDA driver was used for read-only queries. No rename, comment, or save calls were issued.
