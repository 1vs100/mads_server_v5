# Impressions and reporting

An impression is a record that an ad was shown to the player. Impressions are the revenue-relevant events, so both the Lua side and the native side guard them.

## What counts [C, Lua]

`ReportImpression(name, 1)` is called from three places:

| Call site | When |
|---|---|
| `EndPlayVideoAd` (`ads.lua:1120`) | a Massive video finished, when all of the following hold: not a house ad, `bDontReportImpression` false, and no obstruction during the ad |
| `ShowStaticAds` (`ads.lua:1545`) | a static image finished, when not obstructed |
| `ShowStaticAds` (`ads.lua:1647`) | second static path, same rule |

Not reported:

- house ads (`isHouseAd`): `PlayHouseAds`, `LoadHouseAd`;
- the season sponsor (`DontReportSeasonSponsorImpression`), which is set for `ca`, `de`, `fr` and when no sponsor files exist;
- anything obstructed.

## Obstruction [C, Lua]

`MonitorAdObstruction_Task(videoLength)` (`ads.lua:1455`) runs during each Massive ad. `IsAdObstructed()` reports whether the ad is covered by something (a dialog or menu). At the start of an ad `GameVars.Ads.obstructionDuringAd` is set from the current state. An obstruction that occurs during the ad marks it obstructed, and that ad's impression is dropped.

## Native batching [C]

`ImpressionUpdate` requests (type `0xD3`) carry the batch:

- `CRequestImpressionUpdate` (`0x82502BD8`) is created by `CreateImpressionUpdateRequest` (`0x824F4548`). It fails with `0x12D` if the zone has no current MAO state.
- `sub_82502C50` prepends the session pair (final order `+`, `*`), then writes the impression records (`E6`, one per IE name) and the player blocks. Then `sub_82503520` appends the counters `21` and `29` and seals the message.
- The request is sent to `/impsrv/5/activity`.

`XMassiveAdClient::FlushImpressions` (`0x824F1A28`) forces a send. It refuses with `Attempting to flush impressions before internal time has been reached` when the minimum interval has not passed. `Trying to update before internal time has been reached` is the same rule for updates.

The interval comes from the timers set by the locate response (see [Session lifecycle and timers](lifecycle.md)).

## Impression record contents [C]

Each record carries the IE name only:

```text
E6 | u32 (3 + len) | 3D '=' | u16 len | name
```

The name is what the Lua side bound (`Video15_1`, `Video30_2`, `Text_1`, …). The server maps it to the advertiser's record. Any further per-view detail is not carried in the record.

## Counters 21 and 29 [I]

The two `u16` counters written at the end of every impression body come from `this+0x50` and `this+0x52` of the request object. Their meaning is not decoded [U]. They are most likely the number of records in the batch and a second count (for example succeeded or total). All 88 ImpressionUpdates of the v1.0.808.0 capture carried no records and 0 in both counters, which is consistent with that reading but does not prove it.

## Statistics report [C, Lua]

`UpdateAdBreakStats` (`ads.lua:1839`) builds a `LuaErrors` record named `AdBreakStats`, or `LobbyFPP` for the lobby sponsor. It contains:

- `Connected(bool)`, `Zone(name)`, `Enter(time)`, `DLTime(seconds)`;
- `State(code)(text) Error(code)(text)` from `GetState()`;
- a content bitmap `T.1:1 T.2:0 … V30.1:1 … V15.6:0`.

The bitmap is the state of each IE: `T.<n>` for the four static images, `V30.<n>` for the three 30 s videos and `V15.<n>` for the six 15 s videos.

## Statistics request (`0xDA`) [C]

The periodic statistics request (see [Requests](requests.md#statistics-0xda)) carries three `u16` counters from the global block at `dword_82A0EF04`: cache look-ups (`58`), cache hits (`59`) and cache mismatches (`5C`). They are sent to `/impsrv/5/stats`.
