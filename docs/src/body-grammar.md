# Body grammar

Bodies are a flat sequence of **tag bytes** followed by values. There are no nesting markers except explicit length prefixes.

Tags at or above `0xC8` are always followed by a `u32` length. Every reader skips such a tag when it does not know it (`sub_824F96C0`), and fails on an unknown tag below `0xC8`, whose value length it cannot know [C]. `DD`, `DE`, `E3`, `E4`, `E5` and `E6` follow this rule. Writers append to the body buffer unless noted. Some writers **prepend** (the flag argument of the primitive is non-zero), so the final order can differ from the code order.

## Primitives [C]

| Writer | Address | Encoding |
|---|---|---|
| `massive_WriteTag` | `0x824F9EA8` | one byte |
| `massive_WriteU16` | `0x824F9EE8` | `u16 BE` |
| `massive_WriteU64` | `0x824F9F68` | `u64 BE` |
| `massive_WriteStr16` | `0x824FA640` | `u16 BE length` + bytes; the length is `strnlen(s, 2000)`, so no NUL is written and longer strings are cut |
| `u32` writer | `0x824F9F28` | `u32 BE`; used for tags `10` and `11` in EnterZone |
| `massive_BufAppend` | `0x824F99E8` | append raw bytes |
| `massive_BufPrepend` | `0x824F9AA8` | prepend raw bytes |

Reader counterparts: `sub_824F93B0` (`u8`), `sub_824F93F8` (`u16`), `sub_824F9440` (`u32`), `sub_824F9488` (`u64`), `sub_824F94D0` (`f32`), `massive_ReadStr` (`0x824FA168`, `u16` length + bytes), `massive_ReadStrLen` (`0x824F9608`, length and pointer, no copy).

## Player blocks (`massive_WritePlayerUpdateBlocks`, `0x824FA218`) [C]

Up to four slots are read from the client table at `+0x90` (20-byte entries: `u32` occupied flag, then 16 bytes of player id). The loop stops at the first empty slot. Each slot writes:

```text
E4 | 00 00 00 15 | 57 'W' | <slot index u8> | 41 'A' | 00 10 | <16-byte player id>
```

`0x15 = 21` covers the 21 bytes from `W` to the end of the id. The v1.0.808.0 client writes the constant 19 (`0x13`) here for the same 21 bytes, so its player blocks overrun their length by two bytes; the retail writer has the correct value.

## Session fields

Three tags write the session pair used by most bodies [C]:

| Tag | Value | Source |
|---|---|---|
| `2A` `*` | `u64` Massive player id | `qword_82A0EEB0` |
| `2B` `+` | `u64` Massive session id | `qword_82A0EEA8` |

Order: the **code** writes `2B` then `2A` in some bodies and `2A` then `2B` in others. Bodies that prepend `+` and then `*` end with `*` first (CloseSession, PlayerUpdate, Heartbeat). The impression body prepends `*` and then `+`, and the statistics body appends `+` then `*`, so both end with `+` first. Each request chapter gives the final order.

## Tag table (request side)

| Tag | Name | Value | Used in |
|---|---|---|---|
| `3D` `=` | caller string a | `str16` (SKU name) | Locate, OpenSession, zone record, impression record |
| `3E` `>` | caller string b | `str16` (SKU version) | Locate, OpenSession |
| `3C` `<` | session type | `u8`, 1 = single player | OpenSession |
| `18` | encryption algorithm | `u8`, 0 | OpenSession |
| `1B` | hash algorithm | `u8`, 1 | OpenSession |
| `44` `D` | token (the HMAC key string) | `str16("1"×32)` in this build | OpenSession |
| `1D` | hashed hardware id | `str16`, 32 hex characters | Locate, OpenSession (only with a device identity) |
| `16` | AdClient version | `str16("XDK Test QFE 1v1009328.3")` | OpenSession |
| `47` `G` | zone name | `str16(zone)` | EnterZone |
| `2A` `*` | Massive player id | `u64` | most bodies |
| `2B` `+` | Massive session id | `u64` | most bodies |
| `10` | bandwidth total size | `u32` | EnterZone (optional, with `11` and `12`) |
| `11` | bandwidth total time | `u32` | EnterZone (optional, with `10` and `12`) |
| `12` | bandwidth total items | `u16` | EnterZone (optional, with `10` and `11`) |
| `21` `!` | impression count A | `u16` (`this+0x50`) | Impression |
| `29` `)` | impression count B | `u16` (`this+0x52`) | Impression |
| `E6` | impression record | `u32 L` + `3D` + `str16(name)` | Impression |
| `58` `X` | cache look-ups | `u16` (global struct `+0x60`) | Statistics |
| `59` `Y` | cache hits | `u16` (`+0x62`) | Statistics |
| `5C` `\` | cache mismatches | `u16` (`+0x64`) | Statistics |
| `E4` | player block | see above | most bodies |
| `1E` | signature marker | — | sealed bodies |
| `3B` `;` | time marker | `u64` | sealed bodies with time |

## Tag table (response side)

Response tags are listed in [Responses](responses.md) and [Records](records.md).

## Example: a minimal OpenSession body

Assuming a device token and no player blocks:

```text
3D 00 13 "mgs_1_vs_100_x360"
3E 00 04 "edge"
3C 01
18 00
1B 01
44 00 20 "11111111111111111111111111111111"
1D 00 20 "<32 hex characters>"    (the NUL of the 33-byte buffer is not written)
16 00 18 "XDK Test QFE 1v1009328.3"
```

The sealed message then carries `05 CB <L> ... 3B <time64> 1E 00 14 <MAC>`.
