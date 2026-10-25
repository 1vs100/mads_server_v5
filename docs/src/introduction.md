# Introduction

This book documents the **MASSIVE ADS** protocol used by *1 vs 100* (Xbox 360, retail build `v2.0.1149.0.02-2009-10-26`, title `584A07D1`). The game links the Massive Inc. client library (`MassiveAdClient3`, debug names `XMassiveAdClient`, `CRequestManager`, `CMassiveAdObject`, …) and drives it from Lua through a small `Massive` class.

The goal is a complete description of what the game sends and receives, and what it does with the answers: the wire format, the cryptography, the record layouts, the state machine and the Lua-side ad logic.

## What is covered

| Layer | Covered in |
|---|---|
| Lua game logic (zones, ad breaks, impressions, stats) | [Lua integration](lua.md) |
| Native Lua binding and client library | [Architecture](architecture.md) |
| Identity (SKU, console token), init | [Session identity and setup](setup.md) |
| HTTP framing | [Transport](transport.md) |
| Sealing, HMAC, timestamps | [Envelope and cryptography](envelope.md) |
| Tagged field grammar | [Body grammar](body-grammar.md) |
| All 9 request types, bodies and paths | [Requests](requests.md) |
| Response framing and tags | [Responses](responses.md) |
| MAO, order, asset and impression records | [Records](records.md) |
| Asset download and validity checks | [Asset downloads](downloads.md) |
| Impression batching and Lua reporting rules | [Impressions and reporting](impressions.md) |
| Session state, timers, shutdown | [Session lifecycle and timers](lifecycle.md) |
| Asset manifest | [Manifest](manifest.md) |
| Error codes and logs | [Errors and logging](errors.md) |

## Confidence

Every statement is tagged:

- **[C] confirmed**: read directly from the disassembly, the Lua source, or a string in the binary.
- **[I] inferred**: strongly indicated by control flow or naming, not fully traced.
- **[U] unknown**: identified but not decoded. These are collected in [Status](status.md).

## Limits

- No live capture is available and the servers are not reachable from this environment. Wire-format statements are static. The reference code in [Appendix C](app-code.md) is internally consistent (it round-trips its own messages) but has **not** been checked against a real client or server. The field server `massived` exists to close that gap; [Status](status.md) lists what it is expected to answer.
- The server endpoint (host and port) is provided by the game's XLSP lookup on the tag `MASSIVE`. That path is treated as solved and is not re-derived here.
