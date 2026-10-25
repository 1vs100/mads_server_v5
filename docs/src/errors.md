# Errors and logging

## Logging [C]

The library writes diagnostics with the format `printf`-style strings through the client logger (`sub_824F8C08`, called with a numeric code and a message). The game-side log file set is:

| File | Notes |
|---|---|
| `game:\AdClientLog.txt` | client log |
| `game:\AdClient.csv` | status CSV |
| `game:\AdClientMemoryUsage.csv` | memory usage |
| `game:\ClientOverrides.ini` | optional overrides read at start |

The Lua side logs through `printf_L( LogType.Ad, … )` and through `LuaErrors` reports. Two report types go to the server:

- `MASSIVE_INIT`: initialization failure, with the error code (`ads.lua:53`);
- `AdBreakStats` and `LobbyFPP`: statistics (see [Impressions](impressions.md)).

## Numeric error codes seen in the code [C]

| Code | Meaning | Where |
|---|---|---|
| `0x65` | general error (used with most messages) | many |
| `0x69` | invalid initialization structure | `XMassiveAdClient::Initialize` |
| `0xC9` | client already initialized | `XMassiveAdClient::Initialize` |
| `0xD4` | no session to close | `CreateCloseSessionRequest` |
| `0x12D` | could not create impression request (no zone state) | `CreateImpressionUpdateRequest` |
| `0x44C` | NULL builder or buffer | `CreateRequest`, several creators |
| `0x452` | failed to create, add or submit a request | every `Create*Request` |
| `0x258` | asset field `05` is zero | `CreateAsset` |
| `0x25B` | asset URL (field `0C`) missing or empty | `CreateAsset`, `CreateDownloadBinaryRequest` |
| `0x25C` | asset digest (field `04`) missing | `CreateAsset`, `CreateDownloadBinaryRequest` |
| `0x25D` | asset field `07` (expected length) is zero | `CreateAsset`, `CreateDownloadBinaryRequest` |
| `0x25E` | asset field `06` or `02` is zero | `CreateAsset` |

The conditions are [C]. Which message text belongs to which code was not traced; the texts `Invalid asset data length`, `Invalid asset date`, `Invalid asset size` and `Invalid asset ID` are the likely candidates.

Requests that fail to build are reported as `ALLOCATION Failed for CRequest<Name>` (out of memory) or `: Failed To Create / submit / add <Name> Request`.

## Library result strings [C]

Initialization: `Invalid SKU`, `Invalid SKU version`, `Invalid public key`, `Invalid initialization structure`, `AdClient is already initialized`, `AdClient is not initialized`.

Connection: `Could not contact Massive servers`, `Failed to Locate Service: %d`, `Open Session Failed: %d`, `Could not create a new session`, `Could not close the current session, or there was no session to close`, `Cannot join multiple sessions`, `Invalid session ID`.

Zones: `Zone Does Not Exist`, `Zone Name Does not Exist on the Server: %s`, `Cannot Find Zone: %s`, `No zone has been entered`, `Failed to enter zone`, `Failed to exit zone`, `Zone name is unknown or improperly formatted`.

Timing: `Trying to update before internal time has been reached`, `Attempting to flush impressions before internal time has been reached`.

Shutdown: `Shutdown Time out at: %d ms`, `Shutdown already in progress`, `The shutdown process has not completed in an acceptable amount of time`, `A failure in the shutdown process has occurred`.

Transport: `Network bad status`, `Network disconnect error`, `Network connect error`, `Network receive error`, `Network send error`, `Error contacting domain name server`, `Could not resolve the network address`, `Network socket error`.

Assets and content: `Invalid asset data length`, `Invalid asset date`, `Invalid asset size`, `Invalid asset ID`, `Asset data failed validity hash`, `URL is not properly formatted or inaccessible`, `Failed to download asset`, `Failed to Download Asset`, `Asset already exists`, `Failed in Download`, `No Assets for MAO: %s`, `No Assets with ID: %d for Mao: %s`.

Impressions: `Failed to report impressions to server`, `Failed to set impression data`, `Internal Impression data storage error`, `No Impression Record for mao: %s`.

Memory: `Allocation failed: CMemoryBuffer in CMassiveAsset::HandleResponse()`, `Ignoring newly downloaded Massive content as it requires too much memory.`, `Memory allocation failure`.

Versions: `Massive Headers do not match the Version of the Massive library`.

## Lua error flow [C]

`LuaErrors.BuildErrorReport` builds a table `{errorCode, errorName, errorReason, fatalType, programID}`. `LuaErrors.LogErrorReport` sends it to the server. Fatal type `"No"` means the game continues.
