# Requests

Each request is a `CRequestObject` with a type code, a path, a sealing mode, a body writer and a response parser. All request types are listed here.

## Catalogue [C]

| Name | Object code | Path | Type byte | Sealed | Body writer | Response parser |
|---|---|---|---|---|---|---|
| LocateService | `0x1011` | `/adsrv/5/locateService` | `C9` | no | `0x824FEAF8` | `0x824F2E50` (locate branch) |
| OpenSession | `0x1023` | `/adsrv/5/openSession` | `CB` | time + sign | `0x82501818` | `0x825016D8` |
| CloseSession | `0x1073` | `/adsrv/5/closeSession` | `D1` | time + sign | `0x824FE7F0` | `0x824FE718` |
| PlayerUpdate | `0x10A3` | `/adsrv/5/updatePlayers` | `D8` | time + sign | `0x824FF310` | `0x824FF238` |
| EnterZone | `0x1033` | `/adsrv/5/enterZone` | `CD` | time + sign | `0x82503798` | `0x82504728` |
| ImpressionUpdate | `0x1065` | `/impsrv/5/activity` | `D3` | time + sign | `0x82503520` (with `0x82502C50`) | `0x825035D8` |
| Heartbeat | `0x1085` | `/impsrv/5/heartbeat` | `D6` | time + sign | `0x825074E0` | `0x82507408` |
| Statistics | `0x1095` | `/impsrv/5/stats` | `DA` | time + sign | `0x82507800` | `0x82507728` |
| DownloadBinary | `0x54` | asset URL | — (`GET`) | no | none | `0x824F8208` |

Object codes are the constructor arguments passed to `sub_824F9030`. Parsers are the vtable `+0x04` entries. The paths come from vtable `+0x08` (for example `0x82501670` returns `/adsrv/5/openSession`).

Sealed requests are wrapped by `sub_824FA478`; see [Envelope and cryptography](envelope.md).

## Request construction [C]

Every `Create*Request` function follows the same steps:

1. Allocate a `CRequestObject` (`sub_824F3DF8`) and construct it (`RequestXxx`).
2. Check the client is in a state that allows the request (for example OpenSession first closes an existing session).
3. Allocate the body buffer (`CreateRequestBuilder`, sizes 0x100 to 0x400).
4. Write the body with the request's writer.
5. Submit it to `CRequestManager` (`SubmitRequest`, `0x824FAC58`). Failures are logged as `Failed To Create / add / submit <Name> Request`.

## LocateService (`0x1011`) [C]

Unsealed body, no time and no MAC:

```text
3D str16(a)          a = SKU name
3E str16(b)          b = SKU version
1D str16(token)      32 hex characters, only when the device identity exists
```

The response is handled by the locate branch of `0x824F2E50`. It sets the keep-alive timers described in [Session lifecycle and timers](lifecycle.md). The server endpoint itself comes from XLSP (see [Session identity and setup](setup.md)).

## OpenSession (`0x1023`) [C]

Sealed with time and sign. Body in order:

```text
3D str16(a)                 SKU name
3E str16(b)                 SKU version
3C 01                       session type (1 = single player)
18 00                       encryption algorithm (0)
1B 01                       hash algorithm (1)
44 str16("1"×32)            token; the HMAC key string (constant in this build)
[1D str16(token)]           hashed hardware id, only with a device identity
16 str16("XDK Test QFE 1v1009328.3")   AdClient version
<player blocks>             see Body grammar (up to 4)
```

The names of `3C`, `18`, `1B` and `16` are the v1.0.808.0 debug client's log names (`Writing Session Type`, `Writing Encryption Algorithm`, `Writing Hash Algorithm`, `Writing AdClient Version`). That client logged `Connected to Server Singleplayer` after an OpenSession with session type 1. It also writes an optional `1C` string (`Multiplayer GUID`) between `1B` and `44`; the retail writer has no `1C` [C].

Response (`0x825016D8`): section `CC`. Tags `2A` (`*`, Massive player id) and `2B` (`+`, Massive session id) fill the request's `+0x50` and `+0x58`. Those values are then copied to `qword_82A0EEB0` and `qword_82A0EEA8`. Tag `47` (`G`) carries zone records. Details: [Responses](responses.md).

## CloseSession (`0xD1`) [C]

Sealed with time and sign. Written with prepends, so the final order is `*` then `+`:

```text
2A u64(qword_82A0EEB0)      Massive player id
2B u64(qword_82A0EEA8)      Massive session id
```

On a successful response the session id `qword_82A0EEA8` is cleared (the response handler checks for type `0x1073`).

## PlayerUpdate (`0xD8`) [C]

Sealed with time and sign. Body (written with prepends, so the final order is `*` then `+`):

```text
2A u64(qword_82A0EEB0)
2B u64(qword_82A0EEA8)
<player blocks>
```

## EnterZone (`0xCD`) [C]

Sealed with time and sign. Body:

```text
47 'G' str16(zone)          zone name, for example "Lobby_PT"
2A u64(qword_82A0EEB0)
2B u64(qword_82A0EEA8)
<player blocks>
[10 u32]                    bandwidth total size
[11 u32]                    bandwidth total time
[12 u16]                    bandwidth total items
```

The three optional fields are written together, and only when all three source arguments are non-zero [C]. Their names come from the v1.0.808.0 writer (`sub_823E7BF0`: `Writing Bandwidth Total Size`, `Time`, `Items`) [C]. The MAD 3.2 server in `moio9/NFS-Online-Server` reads them as public address, private address and port; that reading is wrong for this protocol revision.

The response (`0x82504728`) carries the MAO list (`DD` records), order blocks (`E3`) and their assets (`DE`). See [Records](records.md).

## ImpressionUpdate (`0xD3`) [C]

Sealed with time and sign. The body is written by `0x82502C50` and then finished by `0x82503520`. The session pair is written with prepends in the order `*`, `+`, so the final order is `+` then `*`, the reverse of the heartbeat:

```text
2B u64(qword_82A0EEA8)
2A u64(qword_82A0EEB0)
E6 u32 (3 + len) '=' str16(ie_name)      one per impression record
...
<player blocks>
21 u16(this+0x50)
29 u16(this+0x52)
```

Impression records and batching are described in [Records](records.md) and [Impressions and reporting](impressions.md).

## Heartbeat (`0xD6`) [C]

Sealed with time and sign. Written with prepends, so the final order is `*` then `+`:

```text
2A u64(qword_82A0EEB0)
2B u64(qword_82A0EEA8)
```

Response parser `0x82507408`.

## Statistics (`0xDA`) [C]

Sealed with time and sign. Written with appends, so the order differs from the heartbeat:

```text
2B u64(qword_82A0EEA8)
2A u64(qword_82A0EEB0)
58 'X' u16                  cache look-ups (dword_82A0EF04 +0x60)
59 'Y' u16                  cache hits (+0x62)
5C '\' u16                  cache mismatches (+0x64)
```

The three counters come from the cache statistics block at `dword_82A0EF04` [C]. The names are the v1.0.808.0 log names (`Writing Cache Look Ups`, `Writing Cache Hits`, `Writing Cache Mismatches`). Its capture sent 12, 6 and 0 after six assets were loaded from cache. Response parser `0x82507728`.

## DownloadBinary (type `0x54`) [C]

A plain `GET` to the asset URL stored in the request. The request has no body. It carries the asset's 20-byte hash as the expected digest (`CreateRequestHash`, `0x82505538`) and the expected length. See [Asset downloads](downloads.md).

## Summary of the session fields per request

| Request | Reads session id | Reads player id | Changes session |
|---|---|---|---|
| LocateService | no | no | no |
| OpenSession | no (closes first if set) | no | sets both |
| CloseSession | yes | yes | clears session id |
| PlayerUpdate | yes | yes | no |
| EnterZone | yes | yes | no |
| ImpressionUpdate | yes | yes | no |
| Heartbeat | yes | yes | no |
| Statistics | yes | yes | no |
| DownloadBinary | no | no | no |
