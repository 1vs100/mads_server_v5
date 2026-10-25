# Session lifecycle and timers

This chapter gathers the state machine from the request handlers, the client update loop, and the Lua calls.

## States [I]

The client state is stored in the object at `+0x10` (`state`) and the request status at the request's `+0x10`. Values written by the code (not yet mapped to names):

| Code | Meaning |
|---|---|
| `0x0B` (11) | written on the open-session / locate response path (`li r11,0xB; stw r11,0x10`) |
| `0x06` (6) | set when a locate or session step is in progress |
| `0x0E` (14) | enter-zone request state, shown in the log as `STATE_ENTERZONE` |
| `0x0F` (15) | enter-zone complete |

The request status values used by the request manager include `REQUEST_STATUS_COMPLETE_PARSED` and the error values `8` and `0xD`. The exact enumeration was not written out [I].

## Sequence

```text
                 Lua Initialize(a,b)
                        │
                        ▼
              XMassiveAdClient::Initialize
              store a,b  ·  compute token33  ·  MassiveThread
                        │
                        ▼
            LocateService (0x1011, C9, unsealed)
          response → locate branch (0x824F2E50)
          timers and flags set, DNS work queued
                        │
                        ▼
            OpenSession (0x1023, CB, sealed)
          response → 0x825016D8
          qB0 = player id, qA8 = session id
                        │
          ┌─────────────┼───────────────────────────────┐
          ▼             ▼                               ▼
   PlayerUpdate     Heartbeat (D6)                 Statistics (DA)
   (D8) on change   periodic                       periodic
                        │
                        ▼
             EnterZone (0x1033, CD, sealed)      ← Lua EnterZone
          response → 0x82504728
          MAO (DD) and order (E3) records → assets
                        │
          ┌─────────────┴─────────────┐
          ▼                           ▼
   DownloadBinary (GET)        ImpressionUpdate (D3)
   per asset, verified         batched, FlushImpressions
                        │
                        ▼
        ExitZone / Shutdown → CloseSession (1073, D1)
        response: qA8 = 0 ;  ShutdownComplete: qA8 = qB0 = 0
```

## Transitions in the code [C]

- **OpenSession** (`CreateOpenSessionRequest`, `0x824F2CE8`): if `qword_82A0EEA8` is non-zero, a CloseSession is created first. A failed open logs `Open Session Failed: %d`.
- **Session established** (`0x825016D8` and the update loop): `qword_82A0EEB0` takes the Massive player id and `qword_82A0EEA8` takes the session id.
- **Session closed** (`0x824F2E50`, type `0x1073`): `qword_82A0EEA8` cleared.
- **Shutdown** (`XMassiveAdClient::ShutdownBegin`, `0x824F2820`): logs `Shutdown Time out at: %d ms`. `ShutdownComplete` (`0x824F0F60`) clears both globals, releases the request manager and the network object, and clears the client pointer `dword_82A0EE0C`. `AbortShutdown` cancels a pending shutdown. `Shutdown already in progress` guards re-entry.
- **Suspend and resume** (`SuspendAll`, `ResumeAll`) are used by the game's pause logic and are not wire-visible [I].

## Timers [C]

The locate response sets three timers (see [Responses](responses.md)). The names are the v1.0.808.0 log names of the same tags:

| Timer | Source | Units | Use |
|---|---|---|---|
| `+0x68` | tag `54` heartbeat period, u8 at locate `+0x80` | minutes × 60 000 ms | heartbeat |
| `+0x78` | tag `3F` statistic period, u8 at locate `+0x88` | minutes × 60 000 ms | statistics |
| `+0x138` | tag `0D` auto reporting time, u16 at locate `+0x7A` | seconds × 1000 | impression flush, enabled by flag `+0x7C` |

The v1.0.808.0 capture shows the effect of `0D` and `3F`. With `0D` = 10 the client logged `Impression Flush: 10000` and sent an ImpressionUpdate every 10 seconds. With `3F` = 10 its single Statistics request came 10 minutes after OpenSession. With `54` = 0 it sent no heartbeat. A retail console with `54` = 1 sent a Heartbeat every 60 s [C]. The retail binding of `3F` and `0D` has not been observed yet [I].

The dword `dword_82A0EEBC` is the client-side timer object. The update function (`XMassiveAdClient::Update`, `0x824F3410`) compares the elapsed time against these deadlines to decide when to send heartbeats, statistics and impression batches. The minimum-interval errors (`Trying to update before internal time…`, `Attempting to flush impressions before internal time…`) are raised when a request is attempted early.

The values of the production server are not known. The Massive test server of the capture sent 0, 10 and 10.

## Time base [I]

The 64-bit time in each sealed envelope comes from `sub_824FE140` (see [Envelope](envelope.md#time-value-i)). It is independent of the timers above.

## Error recovery [I]

- A failed LocateService logs `Failed to Locate Service: %d` and the session attempt stops.
- A failed OpenSession logs `Open Session Failed: %d`.
- Failed requests are removed from the queue (`RemoveRequest`, `0x824FAA58`) and logged.
- Lua-side retries are described in [Lua integration](lua.md): initialization retries after a random 1 to 100 seconds, and zone entry is retried on the next ad break.
