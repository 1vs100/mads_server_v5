# Lua integration

## The Massive class

`onevslive.lua:613` creates the global client object:

```lua
g_Massive = Massive()
```

The native method table at `0x8200CDA0` exposes 14 methods [C]. Each entry is `{name_ptr, cfunc_ptr}`.

| Method | Wrapper | Used by shipped Lua | Purpose |
|---|---|---|---|
| `Initialize(sku, version)` | `0x8218D308` | `ads.lua:47` | create the client with SKU and version |
| `GetResults()` | `0x8218CA68` | `ads.lua:49` | `(ok, errorCode)` after `Initialize` |
| `SetNetBandwidth(kbpsIn, kbpsOut)` | `0x8218CCD0` | `ads.lua:72` | throttle downloads |
| `IsConnected()` | `0x8218CA18` | many | session open |
| `IsInZone()` | `0x8218CBB0` | many | zone entered |
| `EnterZone(name)` | `0x82190728` | `ads.lua:505` | enter a zone (queues the request) |
| `ExitZone()` | `0x82190820` | `ads.lua:494` | leave the zone |
| `BindIE(name, element [, bool])` | `0x82191160` | `ads.lua:510–527`, `1036` | bind an inventory element (starts the download) |
| `UnBindIE(name)` | `0x82190230` | `ads.lua:539–545`, `1049` | release a binding |
| `ReportImpression(name, count)` | `0x8218C978` | `ads.lua:1120`, `1545`, `1647` | record a view |
| `GetState()` | `0x8218CAD0` | `ads.lua:1841` | `{stateCode, stateStr, errorCode, errorStr}` |
| `GetCurrentZoneName()` | `0x8218CBF8` | `ads.lua:693`, `1845` | current zone |
| `UseNetwork(…)` | `0x8218CC78` | no | not used by shipped Lua |
| `Shutdown()` | `0x8218D3A8` | no | not used by shipped Lua |

Wrapper behaviour [C]: each wrapper first resolves the native object from the Lua userdata (`lua_GetMassiveObj`), then reads or updates game-side fields. `EnterZone` compares the requested zone name with the stored name at `+0x2C`, refuses to re-enter the same zone (`Enter the same zone current in!`), and sets the entered flag at `+0xB4`. Only then does it call the library.

## Initialization

`ads.lua:44` (`InitializeMassive`):

1. `g_Massive:Initialize("mgs_1_vs_100_x360", "edge")` (`GameData.MassiveAds.SkuName`, `SkuVersion`, `data.lua:731–732`).
2. `Sync()`, then `GetResults()`.
3. On failure: log a `LuaErrors` report named `MASSIVE_INIT` (fatal = `"No"`), wait `math.random(minInitMassiveWait, maxInitMassiveWait)` seconds (default 1 to 100), and retry as a global task.
4. On success: `SetNetBandwidth(kbps, kbps)` with `massivePTKBPS` (primetime) or `massiveEPKBPS` (extended play), both 7 KB/s by default, overridable from the server configuration (`config.lua:353–392`).

`DEFAULT_MASSIVE_BW_OVERRIDE` (`ads.lua:70`) and `DEFAULT_ENTERZONE_IMMEDIATE` (`ads.lua:423`) are read but never defined in the shipped Lua, so they are nil and inactive in retail [C].

## Staggered start

`InitMassiveAndEnterLobbyZone` (`ads.lua:91`) spreads the calls out so that thousands of consoles do not hit the servers together:

- `gameStartTime` is the round-start cutoff (`PTTimingsTable.RoundStartTable.roleCutoffTime`) plus program start, for primetime (or the EP equivalent).
- Late joiners (`GetAbsoluteTime() > gameStartTime - 30`) move their start to `now + 30`.
- The total window is split in half: one half for `Initialize`, one half for `EnterZone`. Each call waits `random(0, half × 8) / 8` seconds (1/8 s granularity).
- The initial zone is `introVideoAdZoneIndex = 1`. For EP, `STATE_ExtendedPlay.PredictQuestions` picks the zone instead.

## Zones and inventory elements

Data from `data.lua` [C]:

| Item | Values |
|---|---|
| Primetime zones (`ZonesPT`) | `Lobby_PT`, `CommercialBreak1` … `CommercialBreak8` |
| Extended play zones (`ZonesEP`) | `Lobby_EP`, `CommercialBreakEP_1` … `CommercialBreakEP_4` |
| Static image IEs | `Text_1` … `Text_4` |
| 15 s video IEs | `Video15_1` … `Video15_6` |
| 30 s video IEs | `Video30_1` … `Video30_3` |

`WaitToEnterNextAdZone_Task` (`ads.lua:439`) does, in order:

1. Record `currentZone`, `currentZoneName`, `zoneEnterTime`.
2. Wait the randomized enter delay (`minEnterZoneWait` 1 s to `maxEnterZoneWait` 100 s).
3. Wait while a Massive ad is playing.
4. If not connected, initialize now.
5. `ReleaseMassiveIE()` (unbind all), then `ExitZone()` if in a zone.
6. `EnterZone(zoneName)`.
7. `BindIE` the four static images and all video IEs. Binding starts the downloads.

## Ad breaks

`InsertAdBreak` (`ads.lua:569`) drives one break:

- `LoadFirstAd(currentAd)` preloads the first video while the transition plays.
- `AdBreakTransition` plays host commentary.
- `PlaySetAds` plays Massive videos from the bound IEs until the time is used up, falling back to `PlayHouseAds`.
- `ShowStaticAds` shows the four static images (`Text_*`) during the live-host outro.
- `AdBreakReturn` ends the break.

Fallbacks and content sources:

| Function | Content | Impression reported |
|---|---|---|
| `PlaySetAds` | Massive videos (`Video15_*`, `Video30_*`) | yes, unless obstructed |
| `ShowStaticAds` | Massive static images (`Text_*`) | yes, unless obstructed |
| `PlayHouseAds` | local house videos (`HVideo15_*`, `HVideo30_*`) | no (`isHouseAd`) |
| `PlaySponsorAds` | sponsor lists (`SeasonSponsorAdNames*`) | only if `DontReportSeasonSponsorImpression` is false |
| `PlayDiskVideos` | disk-sourced videos | only if the caller allows it |

## Country and sponsor rules

`ads.lua` (around line 250): for `ca`, `de` and `fr`, or when no sponsor files exist on disk, the sponsor lists are replaced by the UserEd and Promo lists, and `DontReportSeasonSponsorImpression = true`. Video files are looked up as `ShowDrive() .. FileDirs.gAdsDir .. name .. ".wmv"`.

## Ad timing

`GameData.AdTimingReset` has 17 entries [C]. Each has `startTime` (minutes), `adTime`, `liveHostPriorTime`, `liveHostPostTime`, `outroStaticAd`, and `zoneIndex`. `InitializeAdTimingIndex` picks the first entry whose start time is in the future. `SetNextAdTimingIndex` enters the zone for that entry if it differs.

## Impressions and statistics

Impression and statistics rules are in [Impressions and reporting](impressions.md). Statistics use `GetState()` and build a bitmap of which slots have content. The bitmap format is `T.<n>:<0|1>` for static images, `V30.<n>` and `V15.<n>` for videos, and it is logged with the zone and download time through `LuaErrors` as `AdBreakStats` or `LobbyFPP`.
