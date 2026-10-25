# Session identity and setup

## Inputs to the client

The client needs three identity values:

| Value | Source | Used by |
|---|---|---|
| SKU name `a` | `Initialize(sku, …)` argument, stored as a pointer at client `+0x158` | `LocateService`, `OpenSession` |
| SKU version `b` | `Initialize(…, version)` argument, stored at client `+0x15C` | `LocateService`, `OpenSession` |
| Device token `token33` | MD5 of the console's MAC address (below) | `LocateService`, `OpenSession` (tag `1D`) |

### How `a` and `b` are set [C]

`XMassiveAdClient::Initialize` (`0x824F3178`) takes an init structure. Its first two words are the SKU name pointer and the version pointer. `sub_824F2368` ("XMassiveAdClient Player IDs") stores them at `+0x158` and `+0x15C`. A NULL structure is rejected with error `0x69` (`Invalid initialization structure`). An already initialized client is rejected with error `0xC9` (`AdClient is already initialized`).

For the shipped game this gives `a = "mgs_1_vs_100_x360"` and `b = "edge"`.

### How `token33` is built [C]

1. `sub_824FC9F8` reads the console's 6-byte hardware (Ethernet) address via `sub_8246AC90` and formats it as uppercase hex with colons: `AA:BB:CC:DD:EE:FF` (17 characters).
2. `sub_82501EF8` (`massive_EncodeToken`) computes `MD5(text)`.
3. The 16-byte digest is printed as lowercase hex with `%08x%08x%08x%08x` into a 33-byte buffer: 32 characters and a NUL.
4. The string writer uses `strnlen`, so the wire carries the 32 characters without the NUL.

```text
token = hex_lower(MD5("AA:BB:CC:DD:EE:FF"))     32 characters on the wire
```

The MAD 3.x write-up derives the same `1D` value from the same MD5 of the hardware-address string, which is an independent confirmation. The token is only produced when the device-identity object exists (`dword_82A0EF08` non-null, `sub_824FC880`). Without it, the optional `1D` field is omitted [C].

The token is a **device identifier, not a secret**: anyone who knows the MAC can compute it.

## Session globals

Two 64-bit globals hold the live session [C]:

| Global | Meaning | Written by |
|---|---|---|
| `qword_82A0EEA8` | session id (`+` field) | OpenSession response (copied from request `+0x58`); cleared by CloseSession response and by shutdown |
| `qword_82A0EEB0` | Massive player id (`*` field) | OpenSession response (copied from request `+0x50`); cleared by shutdown |

`CreateOpenSessionRequest` queues a `CloseSession` first when `qword_82A0EEA8` is non-zero, and logs `No Session to Close` when a close is requested with no session.

## Startup order

```
Lua Initialize(sku, ver)
  └─ XMassiveAdClient::Initialize
       ├─ store a, b (+0x158, +0x15C)
       ├─ compute token33 (device identity)
       └─ start MassiveThread
  └─ LocateService (0x1011, unsealed)           → response handled by 0x824F2E50
  └─ OpenSession   (0x1023, sealed)             → session id and player id stored
  └─ EnterZone …                                 (see Lifecycle)
```

The endpoint host and port come from the game's XLSP lookup on the tag `MASSIVE`. That step is provided by the project and is not part of this protocol.
