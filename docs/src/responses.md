# Responses

Responses use the same envelope as requests. Each response parser starts the same way, and each one requires an exact number of known fields.

## Framing [C]

```text
05                    protocol version, checked and removed by sub_824FA0B8
type                  request type + 1 (table below)
u32 BE L              bytes that follow, the signature trailer included
<fields>
1E                    signature tag (kept in the buffer, covered by the MAC)
00 14                 MAC length
<20-byte MAC>         HMAC-SHA1(key "1"x32) over type | L | fields | 1E
```

- `sub_824FA0B8` reads one byte, removes it from the buffer and returns true only when it is `0x05` (`addi r11,r11,-5; cntlzw`). This is the same byte that starts every request. MAD 3.x uses `0x03` in both directions, so the byte is the protocol revision and matches the `5` in the paths.
- The type byte must equal the value listed below. Any other value fails the parse.
- The signature is read by `sub_824FA108` when the loop meets tag `1E`. It reads a `u16` length and keeps a pointer to a copy of that many bytes in the expected-digest field (`+0x30`). It then removes the length and the MAC bytes from the buffer (`sub_824F9B90`), leaving the `1E` byte in place, and the loop limit `L` is reduced by 22. The length value is not checked; the comparison always uses 20 bytes.
- `sub_824F9300` computes HMAC-SHA1 with the literal key over the remaining buffer and compares 20 bytes. The literal is used directly, so the key does not depend on the OpenSession field `44`.
- An unknown tag at or above `0xC8` is followed by a `u32` length and skipped (`sub_824F96C0`). An unknown tag below `0xC8` fails the parse. A response therefore cannot carry a time field `3B` unless the parser lists it.

## Parsers and acceptance rules [C]

| Request | Response type | Parser | Tags read | Rule |
|---|---|---|---|---|
| LocateService `C9` | `CA` | `sub_824FEBE8` | see below | exactly one `3B`; no MAC |
| OpenSession `CB` | `CC` | `sub_825016D8` | `1E`, `2A`, `2B`, `47` | `1E` + `2A` + `2B` occurrences = 3 |
| EnterZone `CD` | `CE` | `sub_82504728` | `1E`, `25`, `55`, `DD`, `E3` | `1E` + `25` + `55` occurrences = 3; `25` = number of `DD`; `55` = number of `E3` |
| CloseSession `D1` | `D2` | `sub_824FE718` | `1E` | one `1E` |
| ImpressionUpdate `D3` | `D4` | `sub_825035D8` | `1E` | one `1E` |
| Heartbeat `D6` | `D7` | `sub_82507408` | `1E` | one `1E` |
| PlayerUpdate `D8` | `D9` | `sub_824FF238` | `1E` | one `1E` |
| Statistics `DA` | `DB` | `sub_82507728` | `1E` | one `1E` |

The counted totals allow a later occurrence of a field to replace an earlier one, provided the total matches. Every parser verifies the MAC after the loop, except the locate parser, which has no MAC.

## Locate response (`sub_824FEBE8`) [C]

Unsigned. After the version byte the type must be `CA`, then `u32 L`. The loop reads:

| Tag | Reader | Destination | Meaning |
|---|---|---|---|
| `48` | string | address list `+0x50`, index 3 | zone server host |
| `2D` | string | address list `+0x50`, index 4 | media server host |
| `22` | string | address list `+0x50`, index 5 | impression server host |
| `49` | `u16` | port list `+0x60`, index 3 | zone server port |
| `4A` | `u16` | port list `+0x60`, index 4 | media server port |
| `4B` | `u16` | port list `+0x60`, index 5 | impression server port |
| `0D` | `u16` | `+0x7A`, sets `+0x7C` | auto reporting time in seconds (`× 1000`) |
| `3A` | `u16` | `+0x78` | server configurable options, ORed into client `+0x11C`; bit meanings [U] |
| `3B` | `u64` | `+0x70` | server time; counted |
| `3F` | `u8` | `+0x88`, sets `+0x8C` | statistic period in minutes, client timer `+0x78` |
| `54` | `u8` | `+0x80`, sets `+0x84` | heartbeat period in minutes, client timer `+0x68` |
| `56` | `u8` | `+0x90`, sets `+0x94` | logging level |
| `57` | `u8` | discarded | unknown [U] |

The parse succeeds only when exactly one `3B` was read. Address entries are `CAddressIndexPair` objects (`sub_82507208`, index in the third argument); port entries are `CPortIndexPair` objects. The meanings are the log names of the v1.0.808.0 parser (`sub_823D7AE8`), which reads the same tags with the same readers, for example `Reading Heartbeat Period` for `54` and `Reading Statistic Period in Mins` for `3F` [C].

The test server of the v1.0.808.0 capture sent: options 0, heartbeat period 0, statistic period 10, auto reporting time 10, and `tsdev.madserver.net` port 1000 for all three servers. The client logged `Server Configuration: Impression Flush: 10000` and then sent an ImpressionUpdate every 10 seconds.

The completion handler (`0x824F2E50`) converts the stored values into the client timers, see [Session lifecycle and timers](lifecycle.md).

## OpenSession response (`sub_825016D8`) [C]

| Tag | Action |
|---|---|
| `1E` | signature; counted |
| `2A` `*` | `u64` → request `+0x50` (Massive player id); counted |
| `2B` `+` | `u64` → request `+0x58` (Massive session id); counted |
| `47` `G` | string, registered in the zone list at client `+0x160` (`sub_824F1798`, `ZoneNameAdd`); not counted |

The zone list gates zone entry [C]. `XMassiveAdClient::EnterZone` (`0x824F2A18`) looks the requested name up in that list (`sub_824F1728`, case-insensitive). When the list is not empty and the name is missing, it fails with error 300 `Cannot Find Zone: %s` and sends no request. A server must therefore announce every zone the game may enter; the game's zones are listed in [Lua integration](lua.md#zones-and-inventory-elements). A retail console in an Extended Play game sent no EnterZone while only `Lobby_PT` and `CommercialBreak1` were announced, and entered `CommercialBreakEP_2` 1.5 s after OpenSession once all 14 zones were.

On success the globals are updated: `qword_82A0EEB0` takes the player id and `qword_82A0EEA8` takes the session id. The names are the v1.0.808.0 log names (`Reading Massive Player ID`, `Reading Massive Session ID`); its capture received player id 420718 and session id 7417817. MAD 3.x uses `u32` for `2A` and `2B`.

## EnterZone response (`sub_82504728`) [C]

| Tag | Action |
|---|---|
| `1E` | signature; counted |
| `25` `%` | `u16` → `+0x5C`, the number of `DD` records; counted |
| `55` `U` | `u16` → `+0x6C`, the number of `E3` blocks; counted |
| `DD` | MAO record (`sub_82503928`); added to the dynamic creator list `+0x54` when its field `28` is `0x10`, otherwise to the MAO list `+0x58` |
| `E3` | order block (`sub_82504510`), added to the order list `+0x68` |

A record that fails to parse, or whose object has state 0, fails the whole response. Record layouts: [Records](records.md).

## Acknowledgements

The five responses that carry only the signature are 29 bytes long:

```text
05 <type> 00 00 00 17 1E 00 14 <MAC>
```
