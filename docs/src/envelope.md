# Envelope and cryptography

Requests and responses have the same outer shape. The request sealing routine is `sub_824FA478` (`massive_SealEnvelope`) [C]. Responses are described in [Responses](responses.md); they differ only in carrying no time field.

## Layout

```text
05 | type | u32 BE L | body … | [3B | time64] | [1E] | [00 14 | HMAC-SHA1 (20 bytes)]
   └──────────── HMAC covers: type | L | body | [3B time64] | [1E] ───────────┘
```

| Field | Size | Meaning |
|---|---|---|
| `05` | 1 | protocol version, on every message in both directions; responses are rejected unless it is `05` (`sub_824FA0B8`) |
| `type` | 1 | request type byte, see [Requests](requests.md) |
| `L` | 4 | number of bytes that follow this field, **including** `1E` and the 22-byte trailer when signed |
| `body` | n | fields in the grammar of [Body grammar](body-grammar.md) |
| `3B time64` | 9 | `;` marker and a 64-bit time (sealed with time only) |
| `1E` | 1 | signature marker |
| `00 14` | 2 | length of the MAC, always `0x0014` |
| `HMAC-SHA1` | 20 | MAC described below |

## Sealing steps [C]

The body is assembled in an append buffer first. The routine then does the following, in this order:

1. **Time** (`timeFlag`): append `3B` followed by `sub_824FE140()` as a `u64`.
2. **Sign** (`signFlag`): append `1E`.
3. **Length**: prepend `u32 BE L`, where `L = current length + 22` when signed, otherwise `L = current length`.
4. **Type**: prepend the type byte.
5. **MAC** (signed only): `HMAC-SHA1(key, buffer)` over the current buffer, which now holds `type | L | body | [;time] | [1E]`. Append `00 14` and the 20 MAC bytes.
6. **Start marker**: prepend `05`.

The request types use these flags:

| Flags | Used by |
|---|---|
| time + sign | OpenSession `CB`, CloseSession `D1`, PlayerUpdate `D8`, EnterZone `CD`, Impression `D3`, Heartbeat `D6`, Statistics `DA` |
| neither | LocateService `C9` |

## HMAC [C]

- Function: `sub_82505760` (`massive_HMAC_SHA1`). It is a standard RFC 2104 HMAC built on the XeCrypt SHA-1 (`j_XeCryptShaInit`, `j_XeCryptShaUpdate`). Keys longer than 64 bytes are hashed first. The inner pad is `0x36` and the outer pad is `0x5C`.
- Key: the 32-byte ASCII string `"1"×32` (`0x31` repeated). The literal is at `0x8299F640`, in a read/write data segment. It is also written into the OpenSession body as field `44` (see [Requests](requests.md)).

```text
KEY = 31 31 31 … 31   (32 bytes)
```

In this build the key is the constant. Three code paths read the literal and none writes it, and the binary contains no MT19937 code. The MAD 3.x line instead derives a per-session key from the server time, and sends it in the same field (see [Cross-check with MAD 3.x](crosscheck.md)). The constant is not a secret in either case.

The v1.0.808.0 debug client does what the MAD 3.x line does. In its captured session, field `44` held the 30-character value `a750f9ed743bced9553e4f3481545b`, and all 91 signed requests verify with that value as the key and none with `"1"×32` (see [Captured session](capture.md)). A retail console session against `massived` (2026-10-25) confirmed the retail reading: field `44` was `"1"×32` and every request MAC verified with the literal.

## Time value [I]

The 64-bit time comes from `sub_824FE140` (`CMassiveTime::GetTime` in the v1.0.808.0 debug build). It reads a per-client time base at `+0xE0` and computes the value from `sub_824FC880` and `sub_824FC8E8`.

In the v1.0.808.0 capture the value is Unix time in milliseconds: the ImpressionUpdate logged at `2009-04-13 05:41:45,302` carries `1239601305302`, which is that instant. The client logs it as `Server Time(Timestamp)`, which suggests the base is synchronized to the locate server time. The retail build behaves the same: in a session against `massived` its first sealed request carried the locate server time plus 8 ms [C].

## Verification of received messages [C]

The version byte `05` is consumed first by `sub_824FA0B8`. Verification is done by `sub_824F9300` (`massive_VerifyHMAC`):

- It takes the **whole remaining buffer** (`+0x1C` to `+0x24`) and computes HMAC-SHA1 with the same key.
- The expected digest is referenced from `+0x30`: that field holds a pointer to a 20-byte copy. `sub_824FA108` creates the copy from the response's own `1E` signature field (`u16 len` + 20 bytes, through `massive_ReadStrLen`). `sub_824FA108` then **removes** those bytes from the buffer, so the MAC covers everything except the `1E` trailer.
- Asset downloads use a different check: plain SHA-1 of the body against the 20-byte asset hash, not an HMAC (see [Asset downloads](downloads.md)).

Consequences:

- Responses are signed the same way requests are. The `1E` marker stays in the buffer, so it is covered.
- The response MAC does not cover the leading version byte (it is consumed first). The same holds for requests, whose MAC starts at the type byte.

## Hashes used elsewhere

| Hash | Use | Reference |
|---|---|---|
| MD5 | device token (`token33`) | [Session identity and setup](setup.md) |
| HMAC-SHA1 (key `"1"×32`) | envelope MAC and response MAC | this chapter |
| SHA-1 (plain) | asset validity | [Downloads](downloads.md) |
