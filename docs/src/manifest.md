# Manifest

The client reads a manifest named `mani.fest` from the cache area. The parser is `massive_ManifestParse` (`0x82500960`) with a second entry point `0x82500BB8`.

## What is established [C]

- The file is read through a random-access reader. Helper `sub_82505C60` reads `size` bytes at `offset` into a destination buffer, with bounds checks against the file size. Helper `sub_82505948` creates the reader over a buffer with a given size.
- The reader tracks a cursor at `+0x0C` and a total size at `+0x14`. Parsing begins by checking a version or format field at `+0x50`. A value of `2` is rejected by the first check.
- The parser reads 4-byte values with `sub_82505C60` and uses them as offsets and counts. It builds lists with `sub_8250A7E0`, `sub_82505AC0` and `sub_82505B58`.
- Parsing failures are reported through the client error logger.

## What is not decoded [U]

The record layout inside the manifest, the field names, and the meaning of each count were not decoded. The likely purpose is an index of the cached files: names, sizes and validity values. This is inference from the string `mani.fest` next to the cache path and from the presence of assets in the cache. It has not been confirmed.

## Relation to the protocol

The manifest is local. Nothing in the request or response grammar refers to it. It was therefore not needed to build or verify a message.
