# Cross-check with MAD 3.x

Sources: the write-up *Massive Ad Client* (github.com/Aib0t/write-ups, `massive_ad_client/readme.md`), and the MAD 3.2 server in github.com/moio9/NFS-Online-Server (`server/carbon/mad/`), which serves the retail NFS Carbon PC client. It reverse-engineers **MAD 3.2.1.55** on the PC build of *Need for Speed: Carbon* and on the Xbox 360 build of *Skate 2* (with debug symbols). It also reports the MAD 4 XML forms and the leaked SDK version list (2.6.4 to 3.2.1.55). That write-up is an independent source. This chapter compares it with the findings in this book, which are based on the 2009 *1 vs 100* build.

The comparison is useful in two ways. Where both agree, the finding is much stronger. Where they disagree, the two builds are probably at different protocol revisions, and the book says which.

## Agreements

| Topic | MAD 3.x write-up | This book (2009 build) |
|---|---|---|
| Message start | version byte, task id, `u32 BE` length (`03 c9 00000019`), in both directions | `05`, type, `u32 BE L`, in both directions |
| Field tags | `3D` SKU name, `3E` SKU version, `1D` hashed hardware id, `44` token, `16` AdClient version, `2A` player id, `2B` session id, `47` zone, `3B` timestamp, `1E` signature | identical |
| String encoding | tag, `u16 BE` length, bytes | identical (`str16`) |
| Hashed hardware id | `CalculateMD5Hash` of the hardware-address string, hex | `massive_BuildToken33`: MD5 hex of the `XX:XX:…` MAC string |
| Time field | `3B` followed by client time (`u64`) before `1E` | `3B` followed by `u64` |
| Signature | HMAC-SHA1, ipad `0x36`, opad `0x5C` | identical construction |
| Signed region | body **without** the version byte, **with** `1E` at the end | identical (type byte through `1E`) |
| Signature length | `L` plus 23 for the signed region | `L` plus 22 for the trailer, plus `1E` in the counted bytes (same total) |
| Signature trailer | `1E`, `u16 BE` length (20), digest | identical |
| Zone and order blocks | IE block `DD`, order `E3`, counts `25` (IE count) and `55` (order count), `01`, `03`, `13`, `DE` | identical |
| IE sub-tags | `08` angle, `09` distance, `0A` size, `0B` media type, `23`, `24`, `26` id, `27` name, `28` rotation, `2E` | identical set |
| Media type | `0B` (`IEMediaType`) | `0B` chooses the object class |
| Asset expiry | `00` as `u64` | `00` as `u64` |
| Asset URL | `0A` string | `0A` string |
| Locate and opens | `/adsrv/…` paths, `Adclient Massive Inc.` user agent | identical user agent prefix |

The **identity token** is the strongest agreement. The write-up derives the `1D` field from the hardware address and MD5, the same as `massive_BuildToken33` in this book. That confirms the derivation from an independent binary.

Agreements found by the parser review (see [Status](status.md)):

| Topic | MAD 3.x | This book (2009 build) |
|---|---|---|
| Response version byte | `03`, the same byte as requests | `05`, the same byte as requests |
| Locate reply | tags `48`/`49`, `2D`/`4A`, `22`/`4B`, `54`, `3A`, `3B` | the same tags in `sub_824FEBE8`, plus `0D`, `3F`, `56`, `57` |
| Asset record | `02` u64, `04` hash, `05` id, `06`, `07` size, `0A` u16, `0B` type, `0C` url, `15` | identical layout |
| EnterZone fields `10`, `11`, `12` | `10` and `11` u32, `12` u16 | identical layout; moio9 reads them as public address, private address and port, but the v1.0.808.0 writer names them bandwidth total size, time and items |

## Disagreements

| Topic | MAD 3.x write-up | This book (2009 build) | Notes |
|---|---|---|---|
| **HMAC key** | 32 hex characters from MT19937, seeded from the formatted server time, and sent in field `44` | constant `"1"×32` in field `44` and as the key | see below |
| Player and session ids | `2A` and `2B` as `u32` | `2A` and `2B` as `u64` | the 2009 build widened the fields |
| Asset validity | MD5 of the file as a 32-character hex string in field `04` | SHA-1 of the file, first 20 bytes of field `04`, compared byte by byte | different algorithm and encoding, same field |
| Asset size check | size field first | expected length (`+0x48`) first | same order |
| Asset tag `15` | `13` is `CrexId` in the write-up; moio9 sends `15 = 1` to mark the asset downloadable | `15` is the crex id (v1.0.808.0 parser: `Crex ID`); zero disables the `CreateAsset` checks; `13` is not read | the write-up's `13` is the order id, which is `13` in the order block |
| Impression record | XML form in MAD 4, with timestamp, crex, inventory, duration, size and angle | binary record `E6` with the IE name only | the richer record is not in this binary |
| Paths | `/adsrv/locateService`, `/adsrv/openSession` (3.x) | `/adsrv/5/…` and `/impsrv/5/…` | the `5` is the protocol revision in the path |

### The HMAC key

The write-up's MT19937 derivation makes the key a per-session value. Its `MassivePRNG` seeds the generator from the server time string and writes 32 hex characters into a buffer that starts as placeholder text. The write-up then notes that the buffer was "unpopulated" in the decompiler, which matches what we see in the 2009 build.

Our checks on the 2009 build:

- The literal `"1"×32` is at `0x8299F640`. That address is in a segment with read and write permission (`perm 6`, data).
- Three code references read the literal: `massive_VerifyHMAC` (`0x824F931C`), `massive_SealEnvelope` (`0x824FA594`) and `massive_WriteOpenSessionBody` (`0x82501950`). No code was found that writes it.
- The binary does not contain the MT19937 multiplier `0x6C078965` built with `lis`, so no MT19937 code is present.

The v1.0.808.0 debug client of the same game uses a per-session key: its captured requests verify with the 30-character field `44` value, not with `"1"×32` (see [Captured session](capture.md)). A retail console session against `massived` confirmed the constant: the retail build sends `"1"×32` in field `44` and signs with it.

Conclusion for this build: the key is the constant `"1"×32`. This is strong evidence, not proof. A pointer-based write that was not visible to the cross-reference search would not be seen. The reading of the 2009 build is therefore: **constant key**. The 3.x line uses the time-derived key, and the 2009 line appears to predate it or to use a fixed test key. That was the claim in this book's earlier chapters, and it stays, with this qualification.

### Asset validity

The write-up's 3.2 download check compares the MD5 of the file, written as a 32-character hex string, with the asset's `AssetHash` string. The 2009 build compares the plain SHA-1 of the body with 20 bytes stored from the asset record. This is a second, independent difference from 3.x. The earlier notes in this book said HMAC here, which was wrong. The disassembly of `0x825056E0` shows a plain SHA-1.

## What this changes in the book

| Chapter | Change |
|---|---|
| [Envelope](envelope.md) | key described as a constant in this build, with the 3.x difference |
| [Downloads](downloads.md) | digest is plain SHA-1, with a size check first |
| [Records](records.md) | IE and asset field names replaced with the write-up's names where they agree; disagreements listed |
| [Security](security.md) | the key is constant in this build; download check is unkeyed SHA-1 |
| [Status](status.md) | several gaps closed or reframed |

## Things the write-up adds that are not yet in this book

- The EnterZone reply in MAD 4 XML: `mediaList` (id, name, mime type, size, md5sum, urlpath, crex with minimum size, duration and angle), `mediaTarget` (id, name, `mediaRef`), and `mediaZone` (id, name, `mediaTargetRef` list). This is the HTTPS form and is not in the 2009 binary.
- The MAD 4 impression XML (`impressionRecord` with `type`, `timestamp`, `crexId`, `inventoryID`, `duration`, `avgSize`, `avgAngle`).
- The leaked SDK version list (2.6.4.0 through 3.2.1.55) and the statement that the SDK's `.lib` files came with documentation. Those documents were not available for this book.
- The RSA-encrypted verification token in MAD 3.0 (`CRequestObject::WriteU8(0x45)`). The 2009 build writes its token as a string (tag `44`), which the write-up says matches the 3.5 line.

## Method note

The write-up's disassembly was produced with x32dbg and Ghidra on a PC build. The disagreements above come from comparing the two builds, not from trusting one source over the other. Where the write-up gives a field name that our disassembly confirms (for example the IE sub-tags), the name is adopted. Where our reading contradicts it (the asset hash), both readings are recorded.
