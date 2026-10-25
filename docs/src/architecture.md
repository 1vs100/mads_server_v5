# Architecture

```
 Lua game code (ads.lua, statelobby.lua, ...)
        │  g_Massive:Initialize / EnterZone / BindIE / ReportImpression / ...
        ▼
 Lua userdata class "Massive"            lua_GetMassiveObj (0x8218C8D0) [C]
        │  method table @ 0x8200CDA0 [C]
        ▼
 Game-side Massive object                 wrappers 0x8218C978 .. 0x82191160 [C]
        │  keeps its own state: zone name (+0x2C), entered flag (+0xB4) [C]
        │  calls into the library object [I, dispatch partly indirect]
        ▼
 MassiveAdClient3 library                 0x824F0000 – 0x82510000 [C]
   ├ XMassiveAdClient           Initialize / EnterZone / ExitZone / Update / Flush / Shutdown
   ├ CRequestManager            queue, SubmitRequest, pump (sub_824FA8F8)
   ├ CRequestObject (per type)  vtable: +0 dtor, +4 response parser, +8 path getter
   ├ CNetworkManager            server address, port, host name, DNS
   ├ TransactionHTTP            HTTP/1.1 writer and socket (CMassiveSocket)
   ├ CMassiveZoneManager        zones → MAO lists → subscribers
   ├ CMassiveAdObject*          Texture / Audio / Video / Model (dynamic creators)
   ├ CMassiveAsset              asset record, download, validity check
   ├ CMassiveOrder              order record
   ├ CMassiveRecordImpression   impression record
   └ MassiveThread              worker loop (0x82501430)
        │
        ▼
 Massive servers   /adsrv/5/…  (session, zones, players)
                   /impsrv/5/… (impressions, heartbeat, stats)
```

## Request objects

Every request is a `CRequestObject` subclass with a vtable of four entries [C]:

| Vtable slot | Role | Example (open session) |
|---|---|---|
| `+0x00` | deleting destructor | `0x82501680` |
| `+0x04` | response parser | `0x825016D8` |
| `+0x08` | path getter (returns the URL path) | `0x82501670` → `/adsrv/5/openSession` |
| `+0x0C` | base destructor helper | `0x824F90B8` |

The object stores the request type code in its base constructor (`sub_824F9030`). The codes are listed in [Requests](requests.md). The buffer fields used by the append and prepend primitives are `+0x1C` (data pointer), `+0x20` (cursor), `+0x24` (length), `+0x28` (capacity). The expected 20-byte digest is at `+0x30`.

## Threads

`MassiveThread` (`0x82501430`) runs the network loop. Lua-facing calls such as `EnterZone` queue work or set desired state. Lua reads the outcome later through `IsInZone`, `IsConnected` and `GetState`.

## Naming

Functions are renamed in the IDB with two prefixes: `massive_` for the library and `lua_Massive_` for the Lua binding. The full mapping with addresses is in [Appendix A](app-functions.md).
