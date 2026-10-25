# Asset downloads

An asset is one downloadable media file (video, texture, audio or model). Asset records (`DE`) sit inside the order blocks of the EnterZone response, and each is downloaded with a plain HTTP `GET`.

## Lifecycle of an asset [C]

1. **Parsed**: `massive_ReadAssetBlock` validates the record, and `massive_CreateAsset` (`0x824F7BD8`) creates the `CMassiveAsset` (see [Records](records.md#asset-records-de)).
2. **Bound**: the Lua side binds the asset's IE (`BindIE`, see [Lua integration](lua.md)). Binding asks the library to download.
3. **Requested**: `CreateDownloadBinaryRequest` (`0x824F7E90`) checks the expected length (descriptor `+0x28`, asset field `07`; `0x25D` when zero), the URL (descriptor `+0x2C`, field `0C`; `0x25B` when missing or empty) and the digest buffer (asset `+0x6C`, field `04`; `0x25C` when missing), then builds a type `0x54` request. The expected length and the 20-byte digest are passed to `CreateRequestHash` (`0x82505538`), which stores the length at request `+0x48`.
4. **Downloaded**: `RequestDownloadBinary` (`0x82505420`) sends `GET <url>`. The response body is kept in the request buffer.
5. **Verified**: the download parser (`0x82505610`) runs two checks, in this order:
   - **size**: the body length must equal `+0x48`;
   - **digest**: SHA-1 of the body (`sub_825056E0`, plain XeCrypt SHA-1) must equal the 20 bytes at `+0x30`.
6. **Completed**: `CMassiveAsset::HandleResponse` (`0x824F8208`) stores the data, or reports `Failed in Download` / `Failed to Download Asset`.

## What the check is, precisely [C]

- The digest is a **plain SHA-1** of the file, not an HMAC. `sub_825056E0` calls `XeCryptShaInit`, `XeCryptShaUpdate` and the finalizer, with no `0x36` or `0x5C` pads. The earlier notes stated HMAC; that was wrong for this path.
- The stored value is 20 bytes. The check does `memcmp`-style byte comparison of the 20 bytes.
- The **size** check runs first. A file of the wrong size fails before any hashing.

The MAD 3.x write-up describes the same two checks in 3.x, but there the digest is an MD5 hex string of the file (the write-up's `md5sum`, 32 characters). Our build uses SHA-1 over 20 bytes. See [Cross-check with MAD 3.x](crosscheck.md).

## Failure

On mismatch the request reports `Asset data failed validity hash` (in the download error path). The game-side state then waits for the next ad break.

## Buffer and memory [C]

- The download buffer is a `CMemoryBuffer`. If it cannot be allocated, `Allocation failed: CMemoryBuffer in CMassiveAsset::HandleResponse()` is logged.
- Downloads that do not fit in memory are dropped with `Ignoring newly downloaded Massive content as it requires too much memory.` The game-side check uses `Available virtual` and `Available Physical` (logged with `Creating massive content of size %u`).
- Bandwidth is throttled by `SetNetBandwidth`, which sets `XMassiveAdClient::SetMaxReceiveRate` and `SetMaxSendRate`.

## Cache [C]

Downloaded content is written by the file worker (`CFileAsyncWorker`) into the cache directory `game:\Massive`, with cache name `massive-cache-0`. Temporary files use the pattern `%s\%s_%lu.TMP`. Cache jobs are `CCacheJobReadFile`, `CCacheJobWriteFile`, `CCacheJobDeleteFile`, `CCacheJobClearCache` and `CCacheJobCreateCache`. Whether content survives between sessions is [I].

## Completion callbacks [C]

Completion is logged with the name, id and type:

- `A video has completed downloading from Massive. Name (%s) ID (%d) Type (%d)`
- `A texture has completed downloading from Massive. Name (%s) ID (%d) Type (%d)`
- `A fixed product placement (FPP) IE has completed downloading from Massive. Name (%s) ID (%d) Type (%d)`

The Lua side learns about completion through `IsMemoryLoaded` (videos) and `IsAdImageLoaded` (images), which are queried in `BuildContentDownloadedString` and the playback code.
