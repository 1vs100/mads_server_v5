# Security analysis

This chapter lists what the protocol protects and what it does not. It is written from the code paths above. No live traffic was tested.

## Integrity [C]

| Mechanism | What it protects | Strength |
|---|---|---|
| HMAC-SHA1 envelope MAC on requests | integrity of sealed requests | **none against an attacker**: the key is a public constant in this build |
| HMAC-SHA1 response MAC | integrity of sealed responses | same key, same weakness |
| SHA-1 asset check (no key) | integrity of downloaded files | accidental corruption only; anyone who can serve the asset can also serve a matching hash |

The key is the 32-byte string `"1"×32`. It appears in the binary at a fixed address and in every OpenSession body. Anyone who has the game can compute a valid MAC for any message. The MAD 3.x line uses a per-session key derived from the server time, which is a stronger design, but the key is still sent in the same field, so it is not secret from the server either.

## Confidentiality [I]

No TLS was found in the transport (see [Transport](transport.md)). Request bodies contain the SKU, the device token, the session id and the Massive player id, and they travel in cleartext if the transport is plain HTTP. The player id and session id are sufficient to act as the client on the same session.

## Identity [C]

The device token is `MD5("AA:BB:CC:DD:EE:FF")` in lowercase hex (the MAD 3.x write-up derives the same `1D` value) (see [Session identity and setup](setup.md)). It is a fixed function of the console's MAC address. It is not secret, not salted and not bound to the session. It identifies the console to the server.

## Replay [I]

The 64-bit time in each sealed envelope is covered by the MAC. Nothing in the protocol describes a nonce or a checked time window on the client side, so the server would have to enforce replay protection. Whether it does was not tested.

## Authority of the server [I]

The client accepts the server's MAC as proof of the body, using the same public key. It does not authenticate the server. A host that can answer on the expected endpoint can send any response that carries a valid MAC, which any host can compute.

## Build and debug leakage [C]

- The build string `XDK Test QFE 1v1009328.3` is present in every request (`16` field and User-Agent). It identifies a test build of the console SDK.
- Debug strings such as `XMassiveAdClient::EnterZone`, `CRequestManager::SubmitRequest` and `Failed to …` ship in the retail binary.
- The Lua debug switches `DEFAULT_MASSIVE_BW_OVERRIDE` and `DEFAULT_ENTERZONE_IMMEDIATE` are referenced but undefined (inactive in retail).
- `IsNotRetailBuild()` gates debug labels only.

## Denial of service [I]

- Initialization retries use random waits of 1 to 100 seconds.
- Zone entry is staggered by a random window across the start of the game so that consoles do not connect at once.
- The client throttles downloads with `SetNetBandwidth` (7 KB/s by default).

These are operational controls, not security controls.

## Data collected [C]

Impressions carry the IE name. Statistics carry the zone, the download time, the content bitmap and the state codes. Impressions and statistics are sent to the server even when the request body is only partly known. Identifiers are the device token and the SKU.

## Summary

Treat the envelope and asset MACs as checksums. Treat the device token as a stable identifier. Treat every message as readable and forgeable by anyone with the game binary.
