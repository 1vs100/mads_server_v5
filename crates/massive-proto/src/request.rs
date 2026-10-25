//! Requests sent by the client.
//!
//! Encoders reproduce the field order of the retail body writers. Decoders
//! accept any order, reject duplicates and require what the writers always
//! emit.

use crate::crypto::{DeviceToken, MacKey};
use crate::field::Field;
use crate::fieldset::{
    DecodeError, FieldSet, InvalidReason, Result, block_of, bytes_of, str_of, text_of, u8_of,
    u16_of, u32_of, u64_of,
};
use crate::frame::{Frame, FrameBuilder, MessageType, ProtocolVersion, Verified};
use crate::ids::{
    BuildString, ClientTime, IeName, KeyString, MassivePlayerId, PlayerId, PlayerSlot, SessionId,
    SkuName, SkuVersion, ZoneName,
};
use crate::schema::Context;
use crate::tag::Tag;
use crate::wire::{EncodeError, Writer};

const SKU_NAME: Tag = Tag(0x3D);
const SKU_VERSION: Tag = Tag(0x3E);
const DEVICE_TOKEN: Tag = Tag(0x1D);
const SESSION_TYPE: Tag = Tag(0x3C);
const ENCRYPTION_ALGORITHM: Tag = Tag(0x18);
const HASH_ALGORITHM: Tag = Tag(0x1B);
const KEY_STRING: Tag = Tag(0x44);
const BUILD: Tag = Tag(0x16);
const ZONE: Tag = Tag(0x47);
const BANDWIDTH_SIZE: Tag = Tag(0x10);
const BANDWIDTH_TIME: Tag = Tag(0x11);
const BANDWIDTH_ITEMS: Tag = Tag(0x12);
const COUNTER_21: Tag = Tag(0x21);
const COUNTER_29: Tag = Tag(0x29);
const CACHE_LOOKUPS: Tag = Tag(0x58);
const CACHE_HITS: Tag = Tag(0x59);
const CACHE_MISMATCHES: Tag = Tag(0x5C);
const PLAYER_SLOT: Tag = Tag(0x57);
const PLAYER_ID: Tag = Tag(0x41);
const IE_NAME: Tag = Tag(0x3D);

#[derive(Debug, Clone, PartialEq)]
pub struct LocateServiceRequest {
    pub sku_name: SkuName,
    pub sku_version: SkuVersion,
    /// Absent when the client has no device identity object.
    pub device_token: Option<DeviceToken>,
}

/// OpenSession fields `0x3C` session type, `0x18` encryption algorithm and
/// `0x1B` hash algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSessionSettings {
    pub session_type: u8,
    pub encryption_algorithm: u8,
    pub hash_algorithm: u8,
}

impl OpenSessionSettings {
    /// Constants written by `sub_82501818`.
    pub const RETAIL: Self = Self {
        session_type: 1,
        encryption_algorithm: 0,
        hash_algorithm: 1,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenSessionRequest {
    pub sku_name: SkuName,
    pub sku_version: SkuVersion,
    pub settings: OpenSessionSettings,
    pub key_string: KeyString,
    pub device_token: Option<DeviceToken>,
    pub build: BuildString,
    pub players: Vec<PlayerBlock>,
    pub time: ClientTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloseSessionRequest {
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    pub time: ClientTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerUpdateRequest {
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    pub players: Vec<PlayerBlock>,
    pub time: ClientTime,
}

/// EnterZone fields `0x10` total size, `0x11` total time and `0x12` total
/// items. Units are unknown. The writer emits them only when all three source
/// values are non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BandwidthTotals {
    pub size: u32,
    pub time: u32,
    pub items: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnterZoneRequest {
    pub zone: ZoneName,
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    pub players: Vec<PlayerBlock>,
    pub bandwidth: Option<BandwidthTotals>,
    pub time: ClientTime,
}

/// ImpressionUpdate fields `0x21` and `0x29` (request `+0x50`, `+0x52`). Meaning unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpressionCounters {
    pub value_21: u16,
    pub value_29: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImpressionUpdateRequest {
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    /// One `0xE6` record per shown inventory element.
    pub impressions: Vec<IeName>,
    pub players: Vec<PlayerBlock>,
    pub counters: ImpressionCounters,
    pub time: ClientTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeartbeatRequest {
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    pub time: ClientTime,
}

/// Statistics fields `0x58`, `0x59`, `0x5C`: the cache counters at
/// `dword_82A0EF04` `+0x60..+0x64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatisticsCounters {
    pub cache_lookups: u16,
    pub cache_hits: u16,
    pub cache_mismatches: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticsRequest {
    pub session_id: SessionId,
    pub massive_player_id: MassivePlayerId,
    pub counters: StatisticsCounters,
    pub time: ClientTime,
}

/// Player block `0xE4` (`sub_824FA218`): slot index and 16-byte id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBlock {
    pub slot: PlayerSlot,
    pub player_id: PlayerId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    LocateService(LocateServiceRequest),
    OpenSession(OpenSessionRequest),
    CloseSession(CloseSessionRequest),
    PlayerUpdate(PlayerUpdateRequest),
    EnterZone(EnterZoneRequest),
    ImpressionUpdate(ImpressionUpdateRequest),
    Heartbeat(HeartbeatRequest),
    Statistics(StatisticsRequest),
}

/// Session values carried by a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRef {
    pub player_id: MassivePlayerId,
    pub id: SessionId,
}

impl Request {
    #[must_use]
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::LocateService(_) => MessageType::LocateServiceRequest,
            Self::OpenSession(_) => MessageType::OpenSessionRequest,
            Self::CloseSession(_) => MessageType::CloseSessionRequest,
            Self::PlayerUpdate(_) => MessageType::PlayerUpdateRequest,
            Self::EnterZone(_) => MessageType::EnterZoneRequest,
            Self::ImpressionUpdate(_) => MessageType::ImpressionUpdateRequest,
            Self::Heartbeat(_) => MessageType::HeartbeatRequest,
            Self::Statistics(_) => MessageType::StatisticsRequest,
        }
    }

    #[must_use]
    pub fn time(&self) -> Option<ClientTime> {
        match self {
            Self::LocateService(_) => None,
            Self::OpenSession(r) => Some(r.time),
            Self::CloseSession(r) => Some(r.time),
            Self::PlayerUpdate(r) => Some(r.time),
            Self::EnterZone(r) => Some(r.time),
            Self::ImpressionUpdate(r) => Some(r.time),
            Self::Heartbeat(r) => Some(r.time),
            Self::Statistics(r) => Some(r.time),
        }
    }

    #[must_use]
    pub fn session(&self) -> Option<SessionRef> {
        let (player_id, id) = match self {
            Self::LocateService(_) | Self::OpenSession(_) => return None,
            Self::CloseSession(r) => (r.massive_player_id, r.session_id),
            Self::PlayerUpdate(r) => (r.massive_player_id, r.session_id),
            Self::EnterZone(r) => (r.massive_player_id, r.session_id),
            Self::ImpressionUpdate(r) => (r.massive_player_id, r.session_id),
            Self::Heartbeat(r) => (r.massive_player_id, r.session_id),
            Self::Statistics(r) => (r.massive_player_id, r.session_id),
        };
        Some(SessionRef { player_id, id })
    }

    pub fn decode(frame: &Frame<Verified>) -> Result<Self> {
        if frame.version() != ProtocolVersion::RETAIL {
            return Err(DecodeError::WrongVersion {
                expected: ProtocolVersion::RETAIL.0,
                found: frame.version().0,
            });
        }
        let message_type =
            frame
                .message_type()
                .filter(|t| t.is_request())
                .ok_or(DecodeError::UnexpectedType {
                    found: frame.type_byte(),
                })?;
        let context = message_type.context();
        let fields = frame.fields_in(context)?;
        let set = FieldSet::new(context, &fields);
        Ok(match message_type {
            MessageType::LocateServiceRequest => Self::LocateService(LocateServiceRequest {
                sku_name: set.req(SKU_NAME, |s, f| text_of(s, f, SkuName::new))?,
                sku_version: set.req(SKU_VERSION, |s, f| text_of(s, f, SkuVersion::new))?,
                device_token: set.opt(DEVICE_TOKEN, device_token_of)?,
            }),
            MessageType::OpenSessionRequest => Self::OpenSession(OpenSessionRequest {
                sku_name: set.req(SKU_NAME, |s, f| text_of(s, f, SkuName::new))?,
                sku_version: set.req(SKU_VERSION, |s, f| text_of(s, f, SkuVersion::new))?,
                settings: OpenSessionSettings {
                    session_type: set.req(SESSION_TYPE, u8_of)?,
                    encryption_algorithm: set.req(ENCRYPTION_ALGORITHM, u8_of)?,
                    hash_algorithm: set.req(HASH_ALGORITHM, u8_of)?,
                },
                key_string: set.req(KEY_STRING, |s, f| text_of(s, f, KeyString::new))?,
                device_token: set.opt(DEVICE_TOKEN, device_token_of)?,
                build: set.req(BUILD, |s, f| text_of(s, f, BuildString::new))?,
                players: players_of(&set)?,
                time: time_of(&set)?,
            }),
            MessageType::CloseSessionRequest => Self::CloseSession(CloseSessionRequest {
                massive_player_id: player_id_of(&set)?,
                session_id: id_of(&set)?,
                time: time_of(&set)?,
            }),
            MessageType::PlayerUpdateRequest => Self::PlayerUpdate(PlayerUpdateRequest {
                massive_player_id: player_id_of(&set)?,
                session_id: id_of(&set)?,
                players: players_of(&set)?,
                time: time_of(&set)?,
            }),
            MessageType::EnterZoneRequest => Self::EnterZone(EnterZoneRequest {
                zone: set.req(ZONE, |s, f| text_of(s, f, ZoneName::new))?,
                massive_player_id: player_id_of(&set)?,
                session_id: id_of(&set)?,
                players: players_of(&set)?,
                bandwidth: bandwidth_of(&set)?,
                time: time_of(&set)?,
            }),
            MessageType::ImpressionUpdateRequest => {
                Self::ImpressionUpdate(ImpressionUpdateRequest {
                    massive_player_id: player_id_of(&set)?,
                    session_id: id_of(&set)?,
                    impressions: set
                        .all(Tag::IMPRESSION_RECORD)
                        .map(|f| impression_of(&set, f))
                        .collect::<Result<_>>()?,
                    players: players_of(&set)?,
                    counters: ImpressionCounters {
                        value_21: set.req(COUNTER_21, u16_of)?,
                        value_29: set.req(COUNTER_29, u16_of)?,
                    },
                    time: time_of(&set)?,
                })
            }
            MessageType::HeartbeatRequest => Self::Heartbeat(HeartbeatRequest {
                massive_player_id: player_id_of(&set)?,
                session_id: id_of(&set)?,
                time: time_of(&set)?,
            }),
            MessageType::StatisticsRequest => Self::Statistics(StatisticsRequest {
                session_id: id_of(&set)?,
                massive_player_id: player_id_of(&set)?,
                counters: StatisticsCounters {
                    cache_lookups: set.req(CACHE_LOOKUPS, u16_of)?,
                    cache_hits: set.req(CACHE_HITS, u16_of)?,
                    cache_mismatches: set.req(CACHE_MISMATCHES, u16_of)?,
                },
                time: time_of(&set)?,
            }),
            _ => {
                return Err(DecodeError::UnexpectedType {
                    found: frame.type_byte(),
                });
            }
        })
    }

    /// Encodes and seals the request as the retail client would.
    pub fn encode(&self, key: &MacKey) -> std::result::Result<Vec<u8>, EncodeError> {
        let mut w = Writer::new();
        match self {
            Self::LocateService(r) => {
                w.u8(SKU_NAME.0);
                w.str16(r.sku_name.as_str())?;
                w.u8(SKU_VERSION.0);
                w.str16(r.sku_version.as_str())?;
                if let Some(token) = &r.device_token {
                    w.u8(DEVICE_TOKEN.0);
                    w.str16(token.as_str())?;
                }
            }
            Self::OpenSession(r) => {
                w.u8(SKU_NAME.0);
                w.str16(r.sku_name.as_str())?;
                w.u8(SKU_VERSION.0);
                w.str16(r.sku_version.as_str())?;
                w.u8(SESSION_TYPE.0);
                w.u8(r.settings.session_type);
                w.u8(ENCRYPTION_ALGORITHM.0);
                w.u8(r.settings.encryption_algorithm);
                w.u8(HASH_ALGORITHM.0);
                w.u8(r.settings.hash_algorithm);
                w.u8(KEY_STRING.0);
                w.str16(r.key_string.as_str())?;
                if let Some(token) = &r.device_token {
                    w.u8(DEVICE_TOKEN.0);
                    w.str16(token.as_str())?;
                }
                w.u8(BUILD.0);
                w.str16(r.build.as_str())?;
                write_players(&mut w, &r.players)?;
                write_time(&mut w, r.time);
            }
            Self::CloseSession(r) => {
                write_player_id(&mut w, r.massive_player_id);
                write_id(&mut w, r.session_id);
                write_time(&mut w, r.time);
            }
            Self::PlayerUpdate(r) => {
                write_player_id(&mut w, r.massive_player_id);
                write_id(&mut w, r.session_id);
                write_players(&mut w, &r.players)?;
                write_time(&mut w, r.time);
            }
            Self::EnterZone(r) => {
                w.u8(ZONE.0);
                w.str16(r.zone.as_str())?;
                write_player_id(&mut w, r.massive_player_id);
                write_id(&mut w, r.session_id);
                write_players(&mut w, &r.players)?;
                if let Some(bandwidth) = r.bandwidth {
                    w.u8(BANDWIDTH_SIZE.0);
                    w.u32(bandwidth.size);
                    w.u8(BANDWIDTH_TIME.0);
                    w.u32(bandwidth.time);
                    w.u8(BANDWIDTH_ITEMS.0);
                    w.u16(bandwidth.items);
                }
                write_time(&mut w, r.time);
            }
            Self::ImpressionUpdate(r) => {
                write_id(&mut w, r.session_id);
                write_player_id(&mut w, r.massive_player_id);
                for name in &r.impressions {
                    w.block(Tag::IMPRESSION_RECORD.0, |w| {
                        w.u8(IE_NAME.0);
                        w.str16(name.as_str())
                    })?;
                }
                write_players(&mut w, &r.players)?;
                w.u8(COUNTER_21.0);
                w.u16(r.counters.value_21);
                w.u8(COUNTER_29.0);
                w.u16(r.counters.value_29);
                write_time(&mut w, r.time);
            }
            Self::Heartbeat(r) => {
                write_player_id(&mut w, r.massive_player_id);
                write_id(&mut w, r.session_id);
                write_time(&mut w, r.time);
            }
            Self::Statistics(r) => {
                write_id(&mut w, r.session_id);
                write_player_id(&mut w, r.massive_player_id);
                w.u8(CACHE_LOOKUPS.0);
                w.u16(r.counters.cache_lookups);
                w.u8(CACHE_HITS.0);
                w.u16(r.counters.cache_hits);
                w.u8(CACHE_MISMATCHES.0);
                w.u16(r.counters.cache_mismatches);
                write_time(&mut w, r.time);
            }
        }
        FrameBuilder::new(key).build(self.message_type(), w.as_slice())
    }
}

fn device_token_of(set: &FieldSet<'_>, f: &Field) -> Result<DeviceToken> {
    DeviceToken::parse(&str_of(set, f)?).map_err(|e| set.invalid(f, e))
}

fn player_id_of(set: &FieldSet<'_>) -> Result<MassivePlayerId> {
    set.req(Tag::MASSIVE_PLAYER_ID, u64_of).map(MassivePlayerId)
}

fn id_of(set: &FieldSet<'_>) -> Result<SessionId> {
    set.req(Tag::SESSION_ID, u64_of).map(SessionId)
}

fn time_of(set: &FieldSet<'_>) -> Result<ClientTime> {
    set.req(Tag::TIME, u64_of).map(ClientTime)
}

fn bandwidth_of(set: &FieldSet<'_>) -> Result<Option<BandwidthTotals>> {
    if !set.together(&[BANDWIDTH_SIZE, BANDWIDTH_TIME, BANDWIDTH_ITEMS])? {
        return Ok(None);
    }
    Ok(Some(BandwidthTotals {
        size: set.req(BANDWIDTH_SIZE, u32_of)?,
        time: set.req(BANDWIDTH_TIME, u32_of)?,
        items: set.req(BANDWIDTH_ITEMS, u16_of)?,
    }))
}

fn players_of(set: &FieldSet<'_>) -> Result<Vec<PlayerBlock>> {
    set.all(Tag::PLAYER_BLOCK)
        .map(|f| {
            let inner = FieldSet::new(Context::PlayerBlock, block_of(set, f)?);
            let id_field = inner.required(PLAYER_ID)?;
            let raw = bytes_of(&inner, id_field)?;
            let player_id = <[u8; 16]>::try_from(raw).map_err(|_| {
                inner.invalid(
                    id_field,
                    InvalidReason::Length {
                        expected: 16,
                        found: raw.len(),
                    },
                )
            })?;
            Ok(PlayerBlock {
                slot: PlayerSlot(inner.req(PLAYER_SLOT, u8_of)?),
                player_id: PlayerId(player_id),
            })
        })
        .collect()
}

fn impression_of(set: &FieldSet<'_>, f: &Field) -> Result<IeName> {
    let inner = FieldSet::new(Context::ImpressionRecord, block_of(set, f)?);
    inner.req(IE_NAME, |s, f| text_of(s, f, IeName::new))
}

fn write_player_id(w: &mut Writer, player_id: MassivePlayerId) {
    w.u8(Tag::MASSIVE_PLAYER_ID.0);
    w.u64(player_id.get());
}

fn write_id(w: &mut Writer, id: SessionId) {
    w.u8(Tag::SESSION_ID.0);
    w.u64(id.get());
}

fn write_time(w: &mut Writer, time: ClientTime) {
    w.u8(Tag::TIME.0);
    w.u64(time.get());
}

fn write_players(w: &mut Writer, players: &[PlayerBlock]) -> std::result::Result<(), EncodeError> {
    for p in players {
        w.block(Tag::PLAYER_BLOCK.0, |w| {
            w.u8(PLAYER_SLOT.0);
            w.u8(p.slot.get());
            w.u8(PLAYER_ID.0);
            w.bytes16(&p.player_id.0)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::MacAddress;
    use crate::frame::Frame;

    fn token() -> DeviceToken {
        DeviceToken::from_mac_address("00:11:22:AA:BB:CC".parse::<MacAddress>().unwrap())
    }

    fn round_trip(req: &Request) -> Vec<u8> {
        let key = MacKey::retail();
        let bytes = req.encode(&key).unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert_eq!(&Request::decode(&frame).unwrap(), req);
        bytes
    }

    #[test]
    fn locate_matches_retail_layout() {
        let bytes = round_trip(&Request::LocateService(LocateServiceRequest {
            sku_name: SkuName::new("mgs_1_vs_100_x360").unwrap(),
            sku_version: SkuVersion::new("edge").unwrap(),
            device_token: Some(token()),
        }));
        let mut body = b"\x3d\x00\x11mgs_1_vs_100_x360\x3e\x00\x04edge\x1d\x00\x20".to_vec();
        body.extend_from_slice(token().as_str().as_bytes());
        let mut expected = vec![0x05, 0xC9];
        expected.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
        expected.extend_from_slice(&body);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn player_block_matches_retail_bytes() {
        let mut w = Writer::new();
        write_players(
            &mut w,
            &[PlayerBlock {
                slot: PlayerSlot(0),
                player_id: PlayerId([b'A'; 16]),
            }],
        )
        .unwrap();
        let mut expected = b"\xe4\x00\x00\x00\x15W\x00A\x00\x10".to_vec();
        expected.extend_from_slice(&[b'A'; 16]);
        assert_eq!(w.into_inner(), expected);
    }

    #[test]
    fn sealed_requests_round_trip() {
        let player = PlayerBlock {
            slot: PlayerSlot(1),
            player_id: PlayerId([7; 16]),
        };
        let reqs = [
            Request::OpenSession(OpenSessionRequest {
                sku_name: SkuName::new("mgs_1_vs_100_x360").unwrap(),
                sku_version: SkuVersion::new("edge").unwrap(),
                settings: OpenSessionSettings::RETAIL,
                key_string: KeyString::new("1".repeat(32)).unwrap(),
                device_token: Some(token()),
                build: BuildString::new("XDK Test QFE 1v1009328.3").unwrap(),
                players: vec![player],
                time: ClientTime(42),
            }),
            Request::CloseSession(CloseSessionRequest {
                massive_player_id: MassivePlayerId(9),
                session_id: SessionId(3),
                time: ClientTime(1),
            }),
            Request::PlayerUpdate(PlayerUpdateRequest {
                massive_player_id: MassivePlayerId(9),
                session_id: SessionId(3),
                players: vec![player],
                time: ClientTime(2),
            }),
            Request::EnterZone(EnterZoneRequest {
                zone: ZoneName::new("Lobby_PT").unwrap(),
                massive_player_id: MassivePlayerId(9),
                session_id: SessionId(3),
                players: vec![],
                bandwidth: Some(BandwidthTotals {
                    size: 0x0A00_0001,
                    time: 0xC0A8_0001,
                    items: 3074,
                }),
                time: ClientTime(3),
            }),
            Request::ImpressionUpdate(ImpressionUpdateRequest {
                massive_player_id: MassivePlayerId(9),
                session_id: SessionId(3),
                impressions: vec![IeName::new("Video15_1").unwrap()],
                players: vec![player],
                counters: ImpressionCounters {
                    value_21: 1,
                    value_29: 0,
                },
                time: ClientTime(4),
            }),
            Request::Heartbeat(HeartbeatRequest {
                massive_player_id: MassivePlayerId(9),
                session_id: SessionId(3),
                time: ClientTime(5),
            }),
            Request::Statistics(StatisticsRequest {
                session_id: SessionId(3),
                massive_player_id: MassivePlayerId(9),
                counters: StatisticsCounters {
                    cache_lookups: 1,
                    cache_hits: 2,
                    cache_mismatches: 3,
                },
                time: ClientTime(6),
            }),
        ];
        for req in &reqs {
            round_trip(req);
        }
    }

    /// Vectors produced by `work/massive_envelope.py`.
    #[test]
    fn matches_python_reference() {
        let key = MacKey::retail();
        let heartbeat = Request::Heartbeat(HeartbeatRequest {
            massive_player_id: MassivePlayerId(2),
            session_id: SessionId(1),
            time: ClientTime(0),
        });
        assert_eq!(
            hex::encode(heartbeat.encode(&key).unwrap()),
            "05d6000000322a00000000000000022b00000000000000013b00000000000000001e0014\
             b51f51f3fc97556ff3bd56492453ec2e0c5081c3"
        );
        let stats = Request::Statistics(StatisticsRequest {
            session_id: SessionId(3),
            massive_player_id: MassivePlayerId(9),
            counters: StatisticsCounters {
                cache_lookups: 1,
                cache_hits: 2,
                cache_mismatches: 3,
            },
            time: ClientTime(0),
        });
        assert_eq!(
            hex::encode(stats.encode(&key).unwrap()),
            "05da0000003b2b00000000000000032a00000000000000095800015900025c00033b0000000000\
             0000001e00145719bdd76e77fcf26d8b5bc41b77299a80bce42c"
        );
    }

    /// Requests from the v1.0.808.0 debug client's `AdClientLog.txt`. That
    /// build signs with the OpenSession field `44` value.
    #[test]
    fn captured_requests_decode_and_reencode() {
        let key = MacKey::new(*b"a750f9ed743bced9553e4f3481545b");
        let captured = [
            "05cd00000045470010436f6d6d65726369616c427265616b312a0000000000066b6e2b0000000000\
             712fd93b000001209dfecae61e00148ffb7b3a96a40de6243e0ba44aa951a9eb6595e4",
            "05d3000000382b0000000000712fd92a0000000000066b6e2100002900003b000001209dfed6d61e\
             00143e088610c7e8a1f480bfa95df6ee1b850c7903d6",
            "05da0000003b2b0000000000712fd92a0000000000066b6e58000c5900065c00003b000001209e07\
             b3a31e0014ee9b4587237e3b4ecd447c2485a63135ebe23eb9",
        ];
        for hex_bytes in captured {
            let bytes = hex::decode(hex_bytes).unwrap();
            let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
            let req = Request::decode(&frame).unwrap();
            assert_eq!(
                req.session(),
                Some(SessionRef {
                    player_id: MassivePlayerId(420_718),
                    id: SessionId(7_417_817),
                })
            );
            assert_eq!(req.encode(&key).unwrap(), bytes);
        }
    }

    #[test]
    fn statistics_puts_session_id_first() {
        let bytes = round_trip(&Request::Statistics(StatisticsRequest {
            session_id: SessionId(3),
            massive_player_id: MassivePlayerId(9),
            counters: StatisticsCounters {
                cache_lookups: 0,
                cache_hits: 0,
                cache_mismatches: 0,
            },
            time: ClientTime(0),
        }));
        assert_eq!(bytes[6], 0x2B);
        assert_eq!(bytes[15], 0x2A);
    }

    #[test]
    fn partial_bandwidth_totals_are_rejected() {
        let mut w = Writer::new();
        w.u8(ZONE.0);
        w.str16("Lobby_PT").unwrap();
        write_player_id(&mut w, MassivePlayerId(1));
        write_id(&mut w, SessionId(1));
        w.u8(BANDWIDTH_SIZE.0);
        w.u32(1);
        write_time(&mut w, ClientTime(0));
        let key = MacKey::retail();
        let bytes = FrameBuilder::new(&key)
            .build(MessageType::EnterZoneRequest, w.as_slice())
            .unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert!(matches!(
            Request::decode(&frame),
            Err(DecodeError::Incomplete { .. })
        ));
    }
}
