# Appendix A: Function index

Addresses are XEX virtual addresses. Names are the IDB names set during this work (`massive_*` library, `lua_Massive_*` Lua binding). The full closure of 322 library functions with their IDA default names is in `work/asm2/_index.txt`. Per-function listings are in `work/asm2/` (library) and `work/asm/` (annotated key functions).

## Named library and Lua functions

| Address | Name | Size | Role |
|---|---|---|---|
| `8218C8D0` | `lua_GetMassiveObj` | 88 | Lua binding helper: fetch g_Massive native object |
| `8218C978` | `lua_Massive_ReportImpression` | 156 | Lua Massive:ReportImpression wrapper |
| `8218CA18` | `lua_Massive_IsConnected` | 80 | Lua Massive:IsConnected wrapper |
| `8218CA68` | `lua_Massive_GetResults` | 104 | Lua Massive:GetResults wrapper |
| `8218CAD0` | `lua_Massive_GetState` | 220 | Lua Massive:GetState wrapper |
| `8218CBB0` | `lua_Massive_IsInZone` | 68 | Lua Massive:IsInZone wrapper |
| `8218CBF8` | `lua_Massive_GetCurrentZoneName` | 68 | Lua Massive:GetCurrentZoneName wrapper |
| `8218CC78` | `lua_Massive_UseNetwork` | 88 | Lua Massive:UseNetwork wrapper (not used by shipped Lua) |
| `8218CCD0` | `lua_Massive_SetNetBandwidth` | 120 | Lua Massive:SetNetBandwidth wrapper |
| `8218D308` | `lua_Massive_Initialize` | 160 | Lua Massive:Initialize wrapper |
| `8218D3A8` | `lua_Massive_Shutdown` | 180 | Lua Massive:Shutdown wrapper (not used by shipped Lua) |
| `82190230` | `lua_Massive_UnBindIE` | 248 | Lua Massive:UnBindIE wrapper |
| `82190728` | `lua_Massive_EnterZone` | 248 | Lua Massive:EnterZone wrapper |
| `82190820` | `lua_Massive_ExitZone` | 144 | Lua Massive:ExitZone wrapper |
| `82191160` | `lua_Massive_BindIE` | 268 | Lua Massive:BindIE wrapper |
| `824F0448` | `massive_XMassiveAdClientSubscriber_Initialize` | 388 | XMassiveAdClientSubscriber::Initialize |
| `824F0F50` | `massive_GetClientObj` | 12 | Get client object (dword_82A0EE0C) |
| `824F0F60` | `massive_XMassiveAdClient_ShutdownComplete` | 204 | XMassiveAdClient::ShutdownComplete |
| `824F16A8` | `massive_GetDevField158` | 8 | Get owner field 0x158 (session-related value) |
| `824F1A28` | `massive_XMassiveAdClient_FlushImpressions` | 212 | XMassiveAdClient::FlushImpressions |
| `824F1B00` | `massive_CreateCloseSessionRequest` | 328 | CreateCloseSessionRequest |
| `824F1C48` | `massive_CreateLocateServiceRequest` | 312 | CreateLocateServiceRequest |
| `824F1D80` | `massive_CreatePlayerUpdateRequest` | 296 | CreatePlayerUpdateRequest |
| `824F2720` | `massive_SessionCleanup` | 252 |  |
| `824F2820` | `massive_XMassiveAdClient_ShutdownBegin` | 504 | XMassiveAdClient::ShutdownBegin |
| `824F2A18` | `massive_XMassiveAdClient_EnterZone` | 372 | XMassiveAdClient::EnterZone (string 'Cannot Find Zone: %s') |
| `824F2B90` | `massive_XMassiveAdClient_UpdatePlayersInternal` | 340 | XMassiveAdClient::UpdatePlayersInternal |
| `824F2CE8` | `massive_CreateOpenSessionRequest` | 360 | CreateOpenSessionRequest (closes prior session first if qword_82A0EEA8!=0) |
| `824F2E50` | `massive_SessionStateStep` | 744 |  |
| `824F3178` | `massive_XMassiveAdClient_Initialize` | 380 | XMassiveAdClient::Initialize (SKU / public-key validation) |
| `824F32F8` | `j_massive_XMassiveAdClient_ShutdownBegin` | 4 |  |
| `824F3300` | `massive_OpenSession_LocateFail` | 268 | Locate/OpenSession failure handler |
| `824F3410` | `massive_XMassiveAdClient_Update` | 952 | XMassiveAdClient::Update (pump: requests, timers, zone state) |
| `824F37C8` | `massive_XMassiveAdClient_ExitZone` | 364 | XMassiveAdClient::ExitZone |
| `824F3938` | `massive_XMassiveAdClient_UpdatePlayers` | 120 | XMassiveAdClient::UpdatePlayers |
| `824F3A40` | `nullsub_49` | 4 |  |
| `824F3A48` | `nullsub_50` | 4 |  |
| `824F3A50` | `nullsub_51` | 4 |  |
| `824F3A58` | `nullsub_52` | 4 |  |
| `824F3A90` | `massive_BuildToken33` | 136 |  |
| `824F4548` | `massive_CreateImpressionUpdateRequest` | 216 | CreateImpressionUpdateRequest |
| `824F4620` | `massive_ImpressionUpdateSubmit` | 496 | ImpressionUpdate submit/delete |
| `824F49D0` | `massive_CreateEnterZoneRequest` | 548 | CreateEnterZoneRequest |
| `824F4C98` | `massive_AddMAO` | 200 | AddMAO (zone MAO list) |
| `824F4D60` | `massive_AddSubscriberToMAO` | 528 | AddSubscriberToMAO / dynamic MAO |
| `824F53C0` | `massive_HandleResponseMAOList` | 532 | CMassive HandleResponse (MAO list) |
| `824F7BD8` | `massive_CreateAsset` | 576 |  |
| `824F7E90` | `massive_CreateDownloadBinaryRequest` | 492 | CreateDownloadBinaryRequest (expected digest = asset validity hash) |
| `824F8208` | `massive_CMassiveAsset_HandleResponse` | 288 | CMassiveAsset::HandleResponse (download result) |
| `824F9030` | `massive_CRequestObject_ctor` | 136 | CRequestObject base ctor (type code in r4) |
| `824F90C0` | `massive_CRequestObject_dtor` | 172 | CRequestObject base dtor |
| `824F92A8` | `nullsub_53` | 4 |  |
| `824F92B0` | `nullsub_54` | 4 |  |
| `824F9300` | `massive_VerifyHMAC` | 172 | Verify HMAC over buffer vs expected digest (+0x30) |
| `824F9608` | `massive_ReadStrLen` | 180 |  |
| `824F97D0` | `massive_BufAlloc` | 248 | Allocate raw buffer |
| `824F99E8` | `massive_BufAppend` | 192 | Buffer append |
| `824F9AA8` | `massive_BufPrepend` | 228 | Buffer prepend |
| `824F9EA8` | `massive_WriteTag` | 60 | Write tag byte (append/prepend by flag) |
| `824F9EE8` | `massive_WriteU16` | 60 | Write u16 BE |
| `824F9F68` | `massive_WriteU64` | 96 | Write u64 BE |
| `824FA0B8` | `massive_CheckStatusByte` | 80 |  |
| `824FA108` | `massive_ReadSigBytes` | 92 |  |
| `824FA168` | `massive_ReadStr` | 172 |  |
| `824FA218` | `massive_WritePlayerUpdateBlocks` | 344 | CRequestObject::WritePlayerUpdateBlocks |
| `824FA370` | `massive_CreateRequestBuilder` | 124 | CreateRequest builder (raw buffer 0x100/0x200/0x400) |
| `824FA478` | `massive_SealEnvelope` | 456 | Seal / finish envelope (type,time,sign) |
| `824FA640` | `massive_WriteStr16` | 172 | Write u16-length string |
| `824FA6F0` | `massive_WriteImpressionRecordList` | 196 |  |
| `824FA8F8` | `massive_RequestManagerPump` | 104 |  |
| `824FAC58` | `massive_CRequestManager_SubmitRequest` | 264 | CRequestManager::SubmitRequest |
| `824FB080` | `massive_LocateServicePair` | 192 | LocateServicePair (locate.madserver.net) |
| `824FB140` | `massive_DNSResolve` | 220 | Massive DNS resolution |
| `824FB220` | `massive_CNetworkManager_SetServerAddress` | 184 | CNetworkManager::SetServerAddress |
| `824FB358` | `massive_CNetworkManager_SetServerPort` | 104 | CNetworkManager::SetServerPort |
| `824FB648` | `massive_CreateHeartbeatRequest` | 200 |  |
| `824FB720` | `massive_CreateStatisticsRequest` | 200 |  |
| `824FC880` | `massive_GetDeviceIdDword` | 12 |  |
| `824FC9F8` | `massive_ReadConsoleBytes` | 164 |  |
| `824FE140` | `massive_TimeSource` | 64 | CMassiveTime (64-bit timestamp source) |
| `824FE658` | `massive_RequestCloseSession` | 72 | RequestCloseSession (type 0x1073) |
| `824FE7F0` | `massive_WriteCloseSessionBody` | 168 | CloseSession body writer (type 0xD1) |
| `824FE9B0` | `massive_RequestLocateService` | 148 | RequestLocateService (type 0x1011) |
| `824FEAF8` | `massive_WriteLocateBody` | 236 | LocateService body writer (type 0xC9, unsealed) |
| `824FF178` | `massive_RequestPlayerUpdate` | 72 | RequestPlayerUpdate (type 0x10A3) |
| `824FF310` | `massive_WritePlayerUpdateBody` | 184 | PlayerUpdate body writer (tags 2A/2B, type 0xD8) |
| `82500960` | `massive_ManifestParse` | 440 | Manifest parse (mani.fest) |
| `82500BB8` | `massive_ManifestParse2` | 384 | Manifest parse (2) |
| `82501430` | `massive_MassiveThread` | 76 | MassiveThread (worker loop) |
| `82501608` | `massive_RequestOpenSession` | 84 | RequestOpenSession (type 0x1023) |
| `825016D8` | `massive_OpenSession_ParseResp` | 316 | OpenSession response parser (tags 1E/2A/2B/47) |
| `82501818` | `massive_WriteOpenSessionBody` | 528 | OpenSession body writer (type 0xCB) |
| `82501AD0` | `nullsub_55` | 4 |  |
| `82501EF8` | `massive_EncodeToken` | 164 |  |
| `82502BD8` | `massive_RequestImpressionUpdate` | 84 | RequestImpressionUpdate (type 0x1065) |
| `82502C50` | `massive_WriteImpressionRecords` | 236 |  |
| `82503520` | `massive_WriteImpressionBody` | 184 | ImpressionUpdate body writer (tags 21/29, type 0xD3) |
| `825035D8` | `massive_ImpUpd_ParseResp` | 216 | ImpressionUpdate response parser |
| `825036B0` | `massive_ImpUpd_dtor` | 88 | ImpressionUpdate dtor |
| `82503718` | `massive_RequestEnterZone` | 108 | RequestEnterZone (type 0x1033) |
| `82503798` | `massive_WriteEnterZoneBody` | 400 | EnterZone body writer (tag 47 'G', 2A, 2B, blobs 10/11/12; type 0xCD) |
| `82503928` | `massive_ReadMAORecord` | 1196 |  |
| `82503DD8` | `massive_ReadNestedAssetBlock` | 120 |  |
| `82504200` | `massive_ReadAssetBlock` | 700 | ReadAssetBlock (asset record parser) |
| `82504510` | `massive_ReadOrderBlock` | 532 | ReadOrderBlock (tag 0xE3) |
| `82504728` | `massive_ParseResponse` | 860 | Parse (response body; tags 1E/25/55/DD/E3) |
| `82505420` | `massive_RequestDownloadBinary` | 80 | RequestDownloadBinary (type 0x54, HTTP GET) |
| `82505538` | `massive_CreateRequestHash` | 212 | CreateRequestHash (sets expected 20-byte digest) |
| `82505760` | `massive_HMAC_SHA1` | 444 | HMAC-SHA1 (XeCryptSha, ipad 0x36 / opad 0x5C) |
| `82505EC0` | `massive_CMassiveRecordImpression` | 148 | CMassiveRecordImpression |
| `825061F0` | `massive_TransactionHTTP` | 152 | TransactionHTTP (transport object) |
| `82506308` | `massive_BuildHTTPRequest` | 404 | HTTP request builder (POST/GET + headers) |
| `82507348` | `massive_RequestHeartbeat` | 72 | RequestHeartbeat (/impsrv/5/heartbeat) |
| `825074E0` | `massive_WriteHeartbeatFields` | 168 |  |
| `82507588` | `massive_WriteHeartbeatBody` | 100 |  |
| `82507668` | `massive_RequestStatistics` | 72 | RequestStatistics (/impsrv/5/stats) |
| `82507800` | `massive_WriteStatisticsFields` | 396 |  |
| `82507990` | `massive_WriteStatisticsBody` | 100 |  |
| `82508C78` | `nullsub_56` | 4 |  |
| `82509E98` | `nullsub_57` | 4 |  |
| `8250B740` | `massive_CreateOrder` | 80 |  |

## Unnamed functions referenced in this book

| Address | Name in IDB | Role |
|---|---|---|
| `0x824FA0B8` | `massive_CheckStatusByte` | consumes the response status byte, returns true if 0 |
| `0x824FA108` | `massive_ReadSigBytes` | reads the response signature into the expected digest |
| `0x824FA168` | `massive_ReadStr` | reads `u16` length + bytes |
| `0x824F9608` | `massive_ReadStrLen` | reads length and pointer |
| `0x824F93B0` | (reader) | `u8` |
| `0x824F93F8` | (reader) | `u16` |
| `0x824F9440` | (reader) | `u32` |
| `0x824F9488` | (reader) | `u64` |
| `0x824F94D0` | (reader) | `f32` |
| `0x824F9B90` | (remove bytes) | removes bytes at a cursor |
| `0x824F9F28` | (blob writer) | EnterZone blobs `10`, `11` |
| `0x824FE140` | `massive_TimeSource` | 64-bit envelope time |
| `0x824FC880` | `massive_GetDeviceIdDword` | device identity object (`dword_82A0EF08`) |
| `0x824F3A90` | `massive_BuildToken33` | builds the 33-byte token |
| `0x824FC9F8` | `massive_ReadConsoleBytes` | formats the MAC string |
| `0x82501EF8` | `massive_EncodeToken` | MD5 hex |
| `0x8246AC90` | (XNet MAC read) | reads the 6-byte hardware address |
| `0x82505760` | `massive_HMAC_SHA1` | HMAC-SHA1 |
| `0x824AF050` | (memcpy) | copies bytes |
| `0x824B0698` | (sprintf) | formatted output |
| `0x824F8C08` | (client logger) | error code plus message |
| `0x824F3DF8` | (allocator) | object allocation |
| `0x824FE358` | (state helper) | response bookkeeping |
| `0x824FEF78` | (response helper) | response bookkeeping |
| `0x824FF028` | (locate response helper) | locate response |
| `0x82501AD8` | (locate helper) | locate response |
| `0x824F1798` | (zone register) | registers a zone entry |
| `0x824FF238` | (update response) | PlayerUpdate response parser |
| `0x82504728` | `massive_ParseResponse` | EnterZone and update parser |
| `0x82507408` | (heartbeat parser) | heartbeat response |
| `0x82507728` | (stats parser) | statistics response |
| `0x824F8208` | `massive_CMassiveAsset_HandleResponse` | download result |
| `0x82502448` | (Texture constructor) | `CMassiveAdObjectTexture` |
| `0x82502550` | (Audio constructor) | `CMassiveAdObjectAudio` |
| `0x82502648` | (Video constructor) | `CMassiveAdObjectVideo` |
| `0x82502760` | (Model constructor) | `CMassiveAdObjectModel` |
| `0x824F58B0` | (MAO base constructor) | `CMassiveAdObject` |
| `0x82506DD0` | (socket constructor) | `CMassiveSocket` |
| `0x82504158` | (EnterZone helper) | EnterZone body writer dispatch |
| `0x8250B740` | `massive_CreateOrder` | order record |
