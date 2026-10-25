# Appendix B: Tag tables

## Byte-level conventions

| Item | Encoding |
|---|---|
| Integers | big-endian |
| Floats | IEEE 754 single precision, big-endian |
| Strings in bodies | `u16 BE` length, then bytes, no terminator (max 2000) |
| Strings read from responses | the same, or a pointer and length (`ReadStrLen`) |
| Booleans | `u8`, 0 or 1 |

## Envelope bytes

| Byte | Meaning |
|---|---|
| `05` | protocol version, first byte of every message in both directions |
| type | request type, see the table below |
| `3B` `;` | time marker, followed by `u64` |
| `1E` | signature marker, followed by `u16 14` and the MAC |

## Request type bytes

| Type | Request | Sealed |
|---|---|---|
| `C9` | LocateService | no |
| `CB` | OpenSession | time, sign |
| `CD` | EnterZone | time, sign |
| `D1` | CloseSession | time, sign |
| `D3` | ImpressionUpdate | time, sign |
| `D6` | Heartbeat | time, sign |
| `D8` | PlayerUpdate | time, sign |
| `DA` | Statistics | time, sign |
| `E4` | player block (inside bodies, not a request) | — |

## Body tags (request side)

| Tag | Hex | Type | Used by |
|---|---|---|---|
| `=` | `3D` | `str16` caller string a (SKU name) | LocateService, OpenSession, impression record |
| `>` | `3E` | `str16` caller string b (SKU version) | LocateService, OpenSession |
| `<` | `3C` | `u8` session type, 1 = single player | OpenSession |
| — | `18` | `u8` encryption algorithm, 0 | OpenSession |
| — | `1B` | `u8` hash algorithm, 1 | OpenSession |
| — | `1C` | `str16` multiplayer GUID (v1.0.808.0 only) | OpenSession |
| `D` | `44` | `str16("1"×32)` | OpenSession |
| — | `1D` | `str16` hashed hardware id, 32 hex characters | LocateService, OpenSession |
| — | `16` | `str16` AdClient version, `"XDK Test QFE 1v1009328.3"` | OpenSession |
| `G` | `47` | `str16` zone name | EnterZone |
| `*` | `2A` | `u64` Massive player id | most bodies |
| `+` | `2B` | `u64` Massive session id | most bodies |
| — | `10` | `u32` bandwidth total size | EnterZone (optional, all three or none) |
| — | `11` | `u32` bandwidth total time | EnterZone (optional, all three or none) |
| — | `12` | `u16` bandwidth total items | EnterZone (optional, all three or none) |
| `!` | `21` | `u16` impression count A | ImpressionUpdate |
| `)` | `29` | `u16` impression count B | ImpressionUpdate |
| — | `E6` | `u32 L` + `3D` + `str16` name | ImpressionUpdate records |
| `X` | `58` | `u16` cache look-ups | Statistics |
| `Y` | `59` | `u16` cache hits | Statistics |
| `\` | `5C` | `u16` cache mismatches | Statistics |
| — | `E4` | player block | most bodies |

## Response type bytes

| Type | Response | Signed |
|---|---|---|
| `CA` | LocateService | no |
| `CC` | OpenSession | yes |
| `CE` | EnterZone | yes |
| `D2` | CloseSession | yes |
| `D4` | ImpressionUpdate | yes |
| `D7` | Heartbeat | yes |
| `D9` | PlayerUpdate | yes |
| `DB` | Statistics | yes |

## Locate response tags

| Tag | Hex | Type | Meaning |
|---|---|---|---|
| — | `0D` | `u16` | auto reporting time, seconds |
| `"` | `22` | `str16` | impression server host (index 5) |
| `-` | `2D` | `str16` | media server host (index 4) |
| `:` | `3A` | `u16` | server configurable options |
| `;` | `3B` | `u64` | server time, exactly once |
| `?` | `3F` | `u8` | statistic period, minutes |
| `H` | `48` | `str16` | zone server host (index 3) |
| `I` | `49` | `u16` | zone server port |
| `J` | `4A` | `u16` | media server port |
| `K` | `4B` | `u16` | impression server port |
| `T` | `54` | `u8` | heartbeat period, minutes |
| `V` | `56` | `u8` | logging level |
| `W` | `57` | `u8` | unknown, discarded |

## Signed response tags

| Tag | Hex | Type | Where |
|---|---|---|---|
| `1E` | `1E` | signature (`u16 14`, MAC) | all signed responses |
| `*` | `2A` | `u64` Massive player id | OpenSession |
| `+` | `2B` | `u64` Massive session id | OpenSession |
| `G` | `47` | `str16` zone name | OpenSession |
| `%` | `25` | `u16` number of `DD` | EnterZone |
| `U` | `55` | `u16` number of `E3` | EnterZone |
| MAO | `DD` | MAO record | EnterZone |
| order | `E3` | order block | EnterZone |

## Order block sub-tags

| Sub-tag | Hex | Type | Meaning |
|---|---|---|---|
| — | `01` | `u16` | asset count, counted |
| — | `03` | `u32` | frequency cap |
| — | `13` | `u32` | order id, counted |
| asset | `DE` | asset block | one asset |

## MAO record sub-tags

| Sub-tag | Hex | Type | Meaning |
|---|---|---|---|
| — | `08` | `f32` | angle threshold |
| — | `09` | `f32` | distance threshold |
| — | `0A` | `u16` | size threshold, default 95 |
| — | `0B` | `u32` | media type, selects the object class; counted |
| — | `23` | `f32` | adjustment, discarded |
| asset ids | `24` | `u16` count + `u32` array | asset ids; counted |
| id | `26` | `u32` | IE id; counted |
| name | `27` | `str16` | IE name; counted |
| — | `28` | `u16` | rotation type, `0x10` = dynamic creator; counted |
| — | `2E` | `u8` | primary match list size; counted |

## Asset block sub-tags

| Sub-tag | Hex | Type | Meaning |
|---|---|---|---|
| — | `02` | `u64` | expiration, Unix time in ms; counted |
| digest | `04` | `u16` length + bytes | first 20 bytes = SHA-1 of the file; counted when not empty |
| asset id | `05` | `u32` | asset id; counted |
| — | `06` | `u32` | rotation duration, ms; must be non-zero; counted |
| size | `07` | `u32` | exact download length; counted |
| — | `08` | `f32` | angle threshold |
| — | `09` | `f32` | distance threshold |
| — | `0A` | `u16` | size threshold, default 95 |
| type | `0B` | `u32` | asset type; counted |
| url | `0C` | `str16` | download path, for example `/<order>/<crex>/<file>`; counted |
| — | `15` | `u32` | crex (creative) id; zero disables the `CreateAsset` checks; counted |
| — | `E5` | nested block | contains `17` + `str16`, unknown |

## Impression record

| Field | Encoding |
|---|---|
| tag | `E6` |
| length | `u32 BE` = 3 + len(name) |
| `=` | `3D` |
| name | `str16` |

## Statistics fields

The `58`, `59` and `5C` counters are the cache look-ups, hits and mismatches, read from the global block `dword_82A0EF04` at offsets `+0x60`, `+0x62` and `+0x64`.
