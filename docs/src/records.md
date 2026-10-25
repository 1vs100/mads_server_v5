# Records: IE (MAO), orders, assets, impressions

Records are parsed out of the EnterZone response body. The IE (inventory element) records are the **ad placements** in a zone, the order records set the rules for showing them, and the asset records describe the media files to download.

Every record is a block: tag, `u32` length, fields. Each reader loops over its fields, counts some of them, and fails when the count is not exact. Readers skip unknown tags at or above `0xC8` and fail on unknown tags below it.

Field names are the log names of the v1.0.808.0 debug client, whose parsers read the same tags with the same readers (`sub_823E7E68` for IE records, `sub_823E8D38` for orders, `sub_823E84D0` for assets). Example values are from that client's [captured session](capture.md).

## Hierarchy

```text
EnterZone response
 ├─ IE count            (tag 25, u16)  = number of DD records
 ├─ order count         (tag 55, u16)  = number of E3 blocks
 ├─ IE records (DD)     placements: id, name, media type, thresholds, asset ids
 └─ order blocks (E3)   order id, frequency cap, asset count
     └─ asset records (DE)   id, url, digest, size, type, expiry
```

## IE records (`DD`, `sub_82503928`) [C]

| Sub-tag | Reader | Counted | Name | Notes |
|---|---|---|---|---|
| `08` | `f32` | no | `IEAngleThreshold` | default from `flt_82084958` |
| `09` | `f32` | no | `IEDistanceThreshold` | default from `flt_82002120` |
| `0A` | `u16` | no | `IESizeThreshold` | default 95 (`0x5F`) |
| `0B` | `u32` | yes | `IEMediaType` | selects the object class (below) |
| `23` | `f32` | no | `IEAdjustment` | read and discarded |
| `24` | `u16` count + `u32` array (`sub_824F9540`) | yes | `IEAssetArray` | asset ids (asset field `05`) |
| `26` | `u32` | yes | `IEId` | placement id |
| `27` | string | yes | `IEName` | placement name; the Lua side binds by name |
| `28` | `u16` | yes | `IERotationType` | `0x10` puts the record in the dynamic creator list |
| `2E` | `u8` | yes | `IEPrimaryMatchListSize` | stored at object `+0x48` |

The record is accepted only when exactly six counted fields were read. Each id of field `24` is attached to the object with `sub_824F5B68`.

In the capture, the six IE records of zone `CommercialBreak1` had media type 1030 (`0x406`, the video range) for `Video15_*` and `Video30_*` and 3 for `Text_*`, rotation type 2, primary match list size 1, angle threshold 0.57, distance threshold 0.5, size threshold 95 and adjustment 0. Field `24` held asset ids such as 278870, the values of asset field `05`.

### Media type to object class [C]

The compare chain on field `0B` uses signed compares (`cmpwi`):

| Value | Dynamic creator path (`28` = `0x10`) | Static path |
|---|---|---|
| below `0x400` (and every value with the top bit set) | `CMassiveAdObjectTexture` creator, `sub_82502448` | `sub_82501FB0` |
| `0x401` to `0x7FF` | `CMassiveAdObjectVideo` creator, `sub_82502648` | `sub_82502080` |
| `0x801` to `0xFFF` | `CMassiveAdObjectAudio` creator, `sub_82502550` | `sub_824F58B0`, vtable `off_820846CC` |
| above `0x1000` | `CMassiveAdObjectModel` creator, `sub_82502760` | `sub_824F58B0`, vtable `off_82084714` |
| exactly `0x400`, `0x800` or `0x1000` | no object | no object |

The dynamic creator classes are confirmed by their failure strings (`Dynamic CMassiveAdObjectTexture creation failed for IE: "%s"` and the equivalents, `0x820847D8` to `0x820848A8`). The static path classes are [I] by analogy. A boundary value creates no object, so the record and the whole response fail. The Massive test server used 3 for images and 1030 for videos; the values of the production server are not known.

## Order blocks (`E3`, `sub_82504510`) [C]

| Sub-tag | Reader | Counted | Name |
|---|---|---|---|
| `01` | `u16` | yes | `OrderAssetCount`, must equal the number of `DE` records |
| `03` | `u32` | no | `OrderFrequencyCap`, second argument of `CreateOrder`; 100000 in the capture |
| `13` | `u32` | yes | `OrderId`, first argument of `CreateOrder` (`0x8250B740`); also the first path segment of the asset URLs |
| `DE` | asset block | no | `OrderAssetBlock` |

The block is accepted only when exactly two counted fields were read and the asset count matches.

## Asset records (`DE`, `sub_82504200`) [C]

The reader dispatches tags `02` to `0C` through the byte table at `0x82084998`. Each entry is a byte offset from `0x825042C0`. Tags `15` and `E5` are handled outside the table. Tags `00`, `01` and `03` are not handled and fail the record.

| Sub-tag | Reader | Counted | `CreateAsset` argument / descriptor | Name | Meaning |
|---|---|---|---|---|---|
| `02` | `u64` | yes | `r9`, descriptor `+0x10` | `AssetExpiration` | Unix time in ms (1240810902530 in the capture, 14 days after the session) |
| `04` | `u16` length + bytes (`ReadStrLen`) | yes, when not empty | copied to asset `+0x6C` | `AssetHash` | first 20 bytes are compared with the SHA-1 of the download |
| `05` | `u32` | yes | `r4`, descriptor `+0x08` | `AssetId` | the value MAO field `24` refers to |
| `06` | `u32` | yes | `r8`, asset `+0x50` | `AssetRotationDuration` | display time in ms: 10000 for images, 15000 for 15 s videos |
| `07` | `u32` | yes | `r10`, descriptor `+0x28` | `AssetSize` | expected download length |
| `08` | `f32` | no | `f1`, asset `+0x5C` | `AssetAngleThreshold` | |
| `09` | `f32` | no | `f2`, asset `+0x60` | `AssetDistanceThreshold` | |
| `0A` | `u16` | no | asset `+0x5A` | `AssetSizeThreshold` | default 95 |
| `0B` | `u32` | yes | `r6`, descriptor `+0x24` | `AssetType` | same values as the IE media type (3, 1030) |
| `0C` | string | yes | descriptor `+0x2C` | `AssetUrl` | download path, for example `/2607/236126/140912_Text_1_300x250.png` |
| `15` | `u32` | yes | `r5`, descriptor `+0x18` | `CrexId` | creative id, the second path segment of the URL; zero makes `CreateAsset` skip all validity checks |
| `E5` | nested block (`sub_82503DD8`) | no | descriptor `+0x30` | — | contains `17` + string; meaning [U] |

The record is accepted only when exactly eight counted fields were read. An empty field `04` yields no buffer and is not counted.

The layout of `02` to `15` matches the asset records that `moio9/NFS-Online-Server` sends to MAD 3.2.1 clients. The hash differs: MAD 3.2 sends the MD5 of the file as 32 hex characters in field `04`; this build copies 20 bytes and compares them with a SHA-1.

The captured URLs are paths without a host: `/<order id>/<crex id>/<file name>`. The client presumably requests them from the media server named in the locate response. All assets of the capture were loaded from the cache, so no download request was logged, and the SHA-1 reading of field `04` could not be checked against a file.

### Checks in `CreateAsset` (`0x824F7BD8`) [C]

When field `15` is non-zero, `CreateAsset` reports:

| Code | Condition |
|---|---|
| `0x258` | field `05` is zero |
| `0x25D` | field `07` is zero |
| `0x25E` | field `06` is zero or field `02` is zero |
| `0x25B` | field `0C` is missing or empty (logged with `Asset ID: %d`) |
| `0x25C` | field `04` is missing |

Otherwise it allocates 20 bytes at `+0x6C` and copies the digest. Whether an asset that triggered one of these errors is still offered for download is [U]. The message texts assigned to these codes in [Errors and logging](errors.md) were taken from string order and may not match the conditions above.

## Impression records (`E6`) [C]

The 2009 build writes one record per shown IE, containing only its name:

```text
E6 | u32 BE (3 + len(name)) | 3D '=' | u16 len | name bytes
```

Written by `sub_824FA6F0` (`massive_WriteImpressionRecordList`). The name is the IE name the Lua side bound, for example `Video15_1`, `Video30_2` or `Text_1`.

The write-up describes an impression record with more fields (`timestamp`, `crexId`, `inventoryID`, `duration`, `avgSize`, `avgAngle`, and a type of `view` or `impression`). Those were seen in the XML form of the later HTTPS protocol (MAD 4), not in this binary. The 2009 record above does not carry them.

The v1.0.808.0 debug build has binary writers for such reports, which the retail build does not contain. They write a `DF` block per report (`36` report type `u8`, `26` IE id `u32`, `15` crex id `u32`, `32` duration `u32`, `33` and `34` first and last time `u64`, `35` size `u16`, `30` angle `f32` or `31` falloff `f32`, `46` transition count `u16`) and an `E0` block per interaction (`26`, `15`, `33`, `1F` interaction type `u8`, `20` `u16`). All 88 ImpressionUpdates of its capture carried no records.

## Subscriber and zone state [I]

Zones hold MAO lists (`AddMAO`, `0x824F4C98`) and subscribers (`AddSubscriberToMAO`, `0x824F4D60`). These are the in-memory indexes that map a zone's IE records to the IEs the Lua side binds. Their layout was not fully traced.
