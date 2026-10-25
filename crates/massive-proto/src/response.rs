//! Responses sent by the server.
//!
//! Decoders apply the acceptance rules of the retail parsers: the counted
//! field totals, the record count checks and the media type compare chain.
//! They are stricter than the client in one respect: a field the typed model
//! needs must occur exactly once, while the client lets a later occurrence
//! overwrite an earlier one as long as the counted total matches.

use crate::crypto::{AssetDigest, MacKey};
use crate::field::Field;
use crate::fieldset::{
    DecodeError, FieldSet, InvalidReason, Result, block_of, bytes_of, f32_of, str_of, text_of,
    u8_of, u16_of, u32_array_of, u32_of, u64_of,
};
use crate::frame::{Frame, FrameBuilder, MessageType, ProtocolVersion, Verified};
use crate::ids::{
    AngleThreshold, AssetId, AssetType, AssetUrl, ByteCount, CrexId, DistanceThreshold, Expiration,
    FrequencyCap, HostName, IeId, IeName, MassivePlayerId, MediaType, Minutes, OrderId, Port,
    RotationDuration, RotationType, Seconds, ServerOptions, ServerTime, Service, SessionId,
    SizeThreshold, ZoneName,
};
use crate::schema::Context;
use crate::tag::Tag;
use crate::wire::{EncodeError, Writer};

const LOCATE_AUTO_REPORTING_TIME: Tag = Tag(0x0D);
const LOCATE_SERVER_OPTIONS: Tag = Tag(0x3A);
const LOCATE_STATISTIC_PERIOD: Tag = Tag(0x3F);
const LOCATE_HEARTBEAT_PERIOD: Tag = Tag(0x54);
const LOCATE_LOGGING_LEVEL: Tag = Tag(0x56);
const LOCATE_VALUE_57: Tag = Tag(0x57);
const ZONE: Tag = Tag(0x47);
const MAO_COUNT: Tag = Tag(0x25);
const ORDER_COUNT: Tag = Tag(0x55);

const MAO_ANGLE: Tag = Tag(0x08);
const MAO_DISTANCE: Tag = Tag(0x09);
const MAO_SIZE: Tag = Tag(0x0A);
const MAO_MEDIA_TYPE: Tag = Tag(0x0B);
const MAO_ADJUSTMENT: Tag = Tag(0x23);
const MAO_ASSET_IDS: Tag = Tag(0x24);
const MAO_IE_ID: Tag = Tag(0x26);
const MAO_IE_NAME: Tag = Tag(0x27);
const MAO_ROTATION: Tag = Tag(0x28);
const MAO_MATCH_LIST: Tag = Tag(0x2E);
const MAO_COUNTED: [Tag; 6] = [
    MAO_MEDIA_TYPE,
    MAO_ASSET_IDS,
    MAO_IE_ID,
    MAO_IE_NAME,
    MAO_ROTATION,
    MAO_MATCH_LIST,
];

const ORDER_ASSET_COUNT: Tag = Tag(0x01);
const ORDER_FREQUENCY_CAP: Tag = Tag(0x03);
const ORDER_ID: Tag = Tag(0x13);

const ASSET_EXPIRATION: Tag = Tag(0x02);
const ASSET_DIGEST: Tag = Tag(0x04);
const ASSET_ID: Tag = Tag(0x05);
const ASSET_ROTATION_DURATION: Tag = Tag(0x06);
const ASSET_SIZE: Tag = Tag(0x07);
const ASSET_ANGLE: Tag = Tag(0x08);
const ASSET_DISTANCE: Tag = Tag(0x09);
const ASSET_SIZE_THRESHOLD: Tag = Tag(0x0A);
const ASSET_TYPE: Tag = Tag(0x0B);
const ASSET_URL: Tag = Tag(0x0C);
const ASSET_CREX_ID: Tag = Tag(0x15);
const ASSET_EXTRA_VALUE: Tag = Tag(0x17);
const ASSET_COUNTED: [Tag; 8] = [
    ASSET_EXPIRATION,
    ASSET_DIGEST,
    ASSET_ID,
    ASSET_ROTATION_DURATION,
    ASSET_SIZE,
    ASSET_TYPE,
    ASSET_URL,
    ASSET_CREX_ID,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHost {
    pub service: Service,
    pub host: HostName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServicePort {
    pub service: Service,
    pub port: Port,
}

/// Locate response `0xCA` (`sub_824FEBE8`). Unsigned. The client keeps hosts
/// and ports in separate lists keyed by service index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateServiceResponse {
    /// Exactly one occurrence is required.
    pub server_time: ServerTime,
    pub hosts: Vec<ServiceHost>,
    pub ports: Vec<ServicePort>,
    pub auto_reporting_time: Option<Seconds>,
    pub server_options: Option<ServerOptions>,
    /// Stored at request `+0x88`, converted to client timer `+0x78`.
    pub statistic_period: Option<Minutes>,
    /// Stored at request `+0x80`, converted to client timer `+0x68`.
    pub heartbeat_period: Option<Minutes>,
    pub logging_level: Option<u8>,
    /// Read and discarded.
    pub value_57: Option<u8>,
}

/// OpenSession response `0xCC` (`sub_825016D8`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSessionResponse {
    pub massive_player_id: MassivePlayerId,
    pub session_id: SessionId,
    /// Each name is registered in the client zone list (`sub_824F1798`).
    pub zones: Vec<ZoneName>,
}

/// Response that carries only the signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acknowledgement {
    CloseSession,
    ImpressionUpdate,
    Heartbeat,
    PlayerUpdate,
    Statistics,
}

impl Acknowledgement {
    #[must_use]
    pub const fn message_type(self) -> MessageType {
        match self {
            Self::CloseSession => MessageType::CloseSessionResponse,
            Self::ImpressionUpdate => MessageType::ImpressionUpdateResponse,
            Self::Heartbeat => MessageType::HeartbeatResponse,
            Self::PlayerUpdate => MessageType::PlayerUpdateResponse,
            Self::Statistics => MessageType::StatisticsResponse,
        }
    }

    #[must_use]
    pub fn for_response_type(t: MessageType) -> Option<Self> {
        [
            Self::CloseSession,
            Self::ImpressionUpdate,
            Self::Heartbeat,
            Self::PlayerUpdate,
            Self::Statistics,
        ]
        .into_iter()
        .find(|a| a.message_type() == t)
    }
}

/// MAO record `0xDD` (`sub_82503928`): one ad placement.
#[derive(Debug, Clone, PartialEq)]
pub struct MaoRecord {
    pub ie_id: IeId,
    pub ie_name: IeName,
    pub media_type: MediaType,
    pub rotation_type: RotationType,
    pub primary_match_list_size: u8,
    pub asset_ids: Vec<AssetId>,
    pub angle_threshold: Option<AngleThreshold>,
    pub distance_threshold: Option<DistanceThreshold>,
    pub size_threshold: Option<SizeThreshold>,
    /// Read and discarded.
    pub adjustment: Option<f32>,
}

/// Order block `0xE3` (`sub_82504510`).
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub order_id: OrderId,
    pub frequency_cap: Option<FrequencyCap>,
    pub assets: Vec<AssetRecord>,
}

/// Asset record `0xDE` (`sub_82504200`, `CreateAsset` `0x824F7BD8`).
#[derive(Debug, Clone, PartialEq)]
pub struct AssetRecord {
    pub asset_id: AssetId,
    pub expiration: Expiration,
    pub digest: AssetDigest,
    pub rotation_duration: RotationDuration,
    pub size: ByteCount,
    pub asset_type: AssetType,
    pub url: AssetUrl,
    /// Zero makes `CreateAsset` skip every validity check.
    pub crex_id: CrexId,
    pub angle_threshold: Option<AngleThreshold>,
    pub distance_threshold: Option<DistanceThreshold>,
    pub size_threshold: Option<SizeThreshold>,
    /// Field `0x17` of the nested `0xE5` block.
    pub extra: Option<String>,
}

/// Error codes `CreateAsset` reports for a record with `crex_id != 0`.
/// Whether the asset is still used afterwards is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetFault {
    AssetIdZero,
    SizeZero,
    Value06OrExpirationZero,
    UrlEmpty,
}

impl AssetFault {
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::AssetIdZero => 0x258,
            Self::SizeZero => 0x25D,
            Self::Value06OrExpirationZero => 0x25E,
            Self::UrlEmpty => 0x25B,
        }
    }
}

impl AssetRecord {
    #[must_use]
    pub fn faults(&self) -> Vec<AssetFault> {
        if self.crex_id.get() == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.asset_id.get() == 0 {
            out.push(AssetFault::AssetIdZero);
        }
        if self.size.get() == 0 {
            out.push(AssetFault::SizeZero);
        }
        if self.rotation_duration.get() == 0 || self.expiration.get() == 0 {
            out.push(AssetFault::Value06OrExpirationZero);
        }
        if self.url.as_str().is_empty() {
            out.push(AssetFault::UrlEmpty);
        }
        out
    }
}

/// EnterZone response `0xCE` (`sub_82504728`).
#[derive(Debug, Clone, PartialEq)]
pub struct EnterZoneResponse {
    pub maos: Vec<MaoRecord>,
    pub orders: Vec<Order>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    LocateService(LocateServiceResponse),
    OpenSession(OpenSessionResponse),
    EnterZone(EnterZoneResponse),
    Acknowledgement(Acknowledgement),
}

impl Response {
    #[must_use]
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::LocateService(_) => MessageType::LocateServiceResponse,
            Self::OpenSession(_) => MessageType::OpenSessionResponse,
            Self::EnterZone(_) => MessageType::EnterZoneResponse,
            Self::Acknowledgement(a) => a.message_type(),
        }
    }

    pub fn encode(&self, key: &MacKey) -> std::result::Result<Vec<u8>, EncodeError> {
        let mut w = Writer::new();
        match self {
            Self::LocateService(r) => encode_locate(&mut w, r)?,
            Self::OpenSession(r) => {
                w.u8(Tag::MASSIVE_PLAYER_ID.0);
                w.u64(r.massive_player_id.get());
                w.u8(Tag::SESSION_ID.0);
                w.u64(r.session_id.get());
                for zone in &r.zones {
                    w.u8(ZONE.0);
                    w.str16(zone.as_str())?;
                }
            }
            Self::EnterZone(r) => encode_enter_zone(&mut w, r)?,
            Self::Acknowledgement(_) => {}
        }
        FrameBuilder::new(key).build(self.message_type(), w.as_slice())
    }

    /// Decodes a response the way the retail client parses it. The MAC is
    /// checked by [`Frame::verify`] beforehand.
    pub fn decode(frame: &Frame<Verified>) -> Result<Self> {
        if frame.version() != ProtocolVersion::RETAIL {
            return Err(DecodeError::WrongVersion {
                expected: ProtocolVersion::RETAIL.0,
                found: frame.version().0,
            });
        }
        let message_type = frame.message_type().filter(|t| !t.is_request()).ok_or(
            DecodeError::UnexpectedType {
                found: frame.type_byte(),
            },
        )?;
        let context = message_type.context();
        let fields = frame.fields_in(context)?;
        // The trailer is outside the field region; the client counts it with any other 0x1E.
        let signature = usize::from(frame.signature().is_some());
        let set = FieldSet::new(context, &fields);
        Ok(match message_type {
            MessageType::LocateServiceResponse => Self::LocateService(decode_locate(&set)?),
            MessageType::OpenSessionResponse => {
                require_count(
                    &set,
                    &[Tag::MASSIVE_PLAYER_ID, Tag::SESSION_ID, Tag::SIGNATURE],
                    signature,
                    3,
                )?;
                Self::OpenSession(OpenSessionResponse {
                    massive_player_id: MassivePlayerId(set.req(Tag::MASSIVE_PLAYER_ID, u64_of)?),
                    session_id: SessionId(set.req(Tag::SESSION_ID, u64_of)?),
                    zones: set
                        .all(ZONE)
                        .map(|f| text_of(&set, f, ZoneName::new))
                        .collect::<Result<_>>()?,
                })
            }
            MessageType::EnterZoneResponse => {
                require_count(
                    &set,
                    &[MAO_COUNT, ORDER_COUNT, Tag::SIGNATURE],
                    signature,
                    3,
                )?;
                Self::EnterZone(decode_enter_zone(&set)?)
            }
            other => {
                require_count(&set, &[Tag::SIGNATURE], signature, 1)?;
                Self::Acknowledgement(Acknowledgement::for_response_type(other).ok_or(
                    DecodeError::UnexpectedType {
                        found: other.byte(),
                    },
                )?)
            }
        })
    }
}

fn require_count(set: &FieldSet<'_>, tags: &[Tag], extra: usize, required: usize) -> Result<()> {
    let found = set.count(tags) + extra;
    if found == required {
        Ok(())
    } else {
        Err(DecodeError::FieldCount {
            context: set.context,
            required,
            found,
        })
    }
}

fn encode_locate(
    w: &mut Writer,
    r: &LocateServiceResponse,
) -> std::result::Result<(), EncodeError> {
    w.u8(Tag::TIME.0);
    w.u64(r.server_time.get());
    for h in &r.hosts {
        w.u8(h.service.host_tag().0);
        w.str16(h.host.as_str())?;
    }
    for p in &r.ports {
        w.u8(p.service.port_tag().0);
        w.u16(p.port.get());
    }
    if let Some(v) = r.auto_reporting_time {
        w.u8(LOCATE_AUTO_REPORTING_TIME.0);
        w.u16(v.get());
    }
    if let Some(v) = r.server_options {
        w.u8(LOCATE_SERVER_OPTIONS.0);
        w.u16(v.get());
    }
    for (tag, v) in [
        (
            LOCATE_STATISTIC_PERIOD,
            r.statistic_period.map(Minutes::get),
        ),
        (
            LOCATE_HEARTBEAT_PERIOD,
            r.heartbeat_period.map(Minutes::get),
        ),
        (LOCATE_LOGGING_LEVEL, r.logging_level),
        (LOCATE_VALUE_57, r.value_57),
    ] {
        if let Some(v) = v {
            w.u8(tag.0);
            w.u8(v);
        }
    }
    Ok(())
}

fn decode_locate(set: &FieldSet<'_>) -> Result<LocateServiceResponse> {
    require_count(set, &[Tag::TIME], 0, 1)?;
    let mut hosts = Vec::new();
    let mut ports = Vec::new();
    for field in set.fields {
        for service in Service::ALL {
            if field.tag == service.host_tag() {
                hosts.push(ServiceHost {
                    service,
                    host: text_of(set, field, HostName::new)?,
                });
            } else if field.tag == service.port_tag() {
                ports.push(ServicePort {
                    service,
                    port: Port(u16_of(set, field)?),
                });
            }
        }
    }
    Ok(LocateServiceResponse {
        server_time: ServerTime(set.req(Tag::TIME, u64_of)?),
        hosts,
        ports,
        auto_reporting_time: set.opt(LOCATE_AUTO_REPORTING_TIME, u16_of)?.map(Seconds),
        server_options: set.opt(LOCATE_SERVER_OPTIONS, u16_of)?.map(ServerOptions),
        statistic_period: set.opt(LOCATE_STATISTIC_PERIOD, u8_of)?.map(Minutes),
        heartbeat_period: set.opt(LOCATE_HEARTBEAT_PERIOD, u8_of)?.map(Minutes),
        logging_level: set.opt(LOCATE_LOGGING_LEVEL, u8_of)?,
        value_57: set.opt(LOCATE_VALUE_57, u8_of)?,
    })
}

fn encode_enter_zone(
    w: &mut Writer,
    r: &EnterZoneResponse,
) -> std::result::Result<(), EncodeError> {
    w.u8(MAO_COUNT.0);
    w.u16(count_u16(r.maos.len())?);
    w.u8(ORDER_COUNT.0);
    w.u16(count_u16(r.orders.len())?);
    for mao in &r.maos {
        w.block(Tag::MAO_RECORD.0, |w| encode_mao(w, mao))?;
    }
    for order in &r.orders {
        w.block(Tag::ORDER_BLOCK.0, |w| {
            w.u8(ORDER_ASSET_COUNT.0);
            w.u16(count_u16(order.assets.len())?);
            w.u8(ORDER_ID.0);
            w.u32(order.order_id.get());
            if let Some(cap) = order.frequency_cap {
                w.u8(ORDER_FREQUENCY_CAP.0);
                w.u32(cap.get());
            }
            for asset in &order.assets {
                w.block(Tag::ASSET_RECORD.0, |w| encode_asset(w, asset))?;
            }
            Ok::<_, EncodeError>(())
        })?;
    }
    Ok(())
}

fn count_u16(n: usize) -> std::result::Result<u16, EncodeError> {
    u16::try_from(n).map_err(|_| EncodeError::TooLong {
        len: n,
        max: usize::from(u16::MAX),
    })
}

fn encode_mao(w: &mut Writer, m: &MaoRecord) -> std::result::Result<(), EncodeError> {
    w.u8(MAO_MEDIA_TYPE.0);
    w.u32(m.media_type.0);
    w.u8(MAO_ASSET_IDS.0);
    let ids: Vec<u32> = m.asset_ids.iter().map(|a| a.get()).collect();
    w.u32_array16(&ids)?;
    w.u8(MAO_IE_ID.0);
    w.u32(m.ie_id.get());
    w.u8(MAO_IE_NAME.0);
    w.str16(m.ie_name.as_str())?;
    w.u8(MAO_ROTATION.0);
    w.u16(m.rotation_type.0);
    w.u8(MAO_MATCH_LIST.0);
    w.u8(m.primary_match_list_size);
    write_thresholds(w, m.angle_threshold, m.distance_threshold, m.size_threshold);
    if let Some(v) = m.adjustment {
        w.u8(MAO_ADJUSTMENT.0);
        w.f32(v);
    }
    Ok(())
}

fn encode_asset(w: &mut Writer, a: &AssetRecord) -> std::result::Result<(), EncodeError> {
    w.u8(ASSET_EXPIRATION.0);
    w.u64(a.expiration.get());
    w.u8(ASSET_DIGEST.0);
    w.bytes16(&a.digest.0)?;
    w.u8(ASSET_ID.0);
    w.u32(a.asset_id.get());
    w.u8(ASSET_ROTATION_DURATION.0);
    w.u32(a.rotation_duration.get());
    w.u8(ASSET_SIZE.0);
    w.u32(a.size.get());
    write_thresholds(w, a.angle_threshold, a.distance_threshold, a.size_threshold);
    w.u8(ASSET_TYPE.0);
    w.u32(a.asset_type.get());
    w.u8(ASSET_URL.0);
    w.str16(a.url.as_str())?;
    w.u8(ASSET_CREX_ID.0);
    w.u32(a.crex_id.get());
    if let Some(extra) = &a.extra {
        w.block(Tag::ASSET_EXTRA.0, |w| {
            w.u8(ASSET_EXTRA_VALUE.0);
            w.str16(extra)
        })?;
    }
    Ok(())
}

/// Tags `0x08`, `0x09`, `0x0A` have the same encoding in MAO and asset records.
fn write_thresholds(
    w: &mut Writer,
    angle: Option<AngleThreshold>,
    distance: Option<DistanceThreshold>,
    size: Option<SizeThreshold>,
) {
    if let Some(v) = angle {
        w.u8(MAO_ANGLE.0);
        w.f32(v.0);
    }
    if let Some(v) = distance {
        w.u8(MAO_DISTANCE.0);
        w.f32(v.0);
    }
    if let Some(v) = size {
        w.u8(MAO_SIZE.0);
        w.u16(v.get());
    }
}

fn decode_enter_zone(set: &FieldSet<'_>) -> Result<EnterZoneResponse> {
    let maos = set
        .all(Tag::MAO_RECORD)
        .map(|f| decode_mao(&FieldSet::new(Context::MaoRecord, block_of(set, f)?)))
        .collect::<Result<Vec<_>>>()?;
    let orders = set
        .all(Tag::ORDER_BLOCK)
        .map(|f| decode_order(&FieldSet::new(Context::OrderBlock, block_of(set, f)?)))
        .collect::<Result<Vec<_>>>()?;
    check_declared(set, MAO_COUNT, maos.len())?;
    check_declared(set, ORDER_COUNT, orders.len())?;
    Ok(EnterZoneResponse { maos, orders })
}

fn check_declared(set: &FieldSet<'_>, tag: Tag, actual: usize) -> Result<()> {
    let declared = set.req(tag, u16_of)?;
    if usize::from(declared) == actual {
        Ok(())
    } else {
        Err(DecodeError::CountMismatch {
            context: set.context,
            tag,
            declared,
            actual,
        })
    }
}

fn decode_mao(set: &FieldSet<'_>) -> Result<MaoRecord> {
    require_count(set, &MAO_COUNTED, 0, MAO_COUNTED.len())?;
    let media_field = set.required(MAO_MEDIA_TYPE)?;
    let media_type = MediaType(u32_of(set, media_field)?);
    if media_type.class().is_none() {
        return Err(set.invalid(media_field, InvalidReason::MediaType(media_type.0)));
    }
    Ok(MaoRecord {
        ie_id: IeId(set.req(MAO_IE_ID, u32_of)?),
        ie_name: set.req(MAO_IE_NAME, |s, f| text_of(s, f, IeName::new))?,
        media_type,
        rotation_type: RotationType(set.req(MAO_ROTATION, u16_of)?),
        primary_match_list_size: set.req(MAO_MATCH_LIST, u8_of)?,
        asset_ids: set
            .req(MAO_ASSET_IDS, u32_array_of)?
            .iter()
            .copied()
            .map(AssetId)
            .collect(),
        angle_threshold: set.opt(MAO_ANGLE, f32_of)?.map(AngleThreshold),
        distance_threshold: set.opt(MAO_DISTANCE, f32_of)?.map(DistanceThreshold),
        size_threshold: set.opt(MAO_SIZE, u16_of)?.map(SizeThreshold),
        adjustment: set.opt(MAO_ADJUSTMENT, f32_of)?,
    })
}

fn decode_order(set: &FieldSet<'_>) -> Result<Order> {
    require_count(set, &[ORDER_ASSET_COUNT, ORDER_ID], 0, 2)?;
    let assets = set
        .all(Tag::ASSET_RECORD)
        .map(|f| decode_asset(&FieldSet::new(Context::AssetRecord, block_of(set, f)?)))
        .collect::<Result<Vec<_>>>()?;
    check_declared(set, ORDER_ASSET_COUNT, assets.len())?;
    Ok(Order {
        order_id: OrderId(set.req(ORDER_ID, u32_of)?),
        frequency_cap: set.opt(ORDER_FREQUENCY_CAP, u32_of)?.map(FrequencyCap),
        assets,
    })
}

fn decode_asset(set: &FieldSet<'_>) -> Result<AssetRecord> {
    // ReadStrLen yields no buffer for an empty digest, which is then not counted.
    let empty_digests = set
        .all(ASSET_DIGEST)
        .filter(|f| bytes_of(set, f).is_ok_and(<[u8]>::is_empty))
        .count();
    let found = set.count(&ASSET_COUNTED) - empty_digests;
    if found != ASSET_COUNTED.len() {
        return Err(DecodeError::FieldCount {
            context: set.context,
            required: ASSET_COUNTED.len(),
            found,
        });
    }
    Ok(AssetRecord {
        asset_id: AssetId(set.req(ASSET_ID, u32_of)?),
        expiration: Expiration(set.req(ASSET_EXPIRATION, u64_of)?),
        digest: set.req(ASSET_DIGEST, digest_of)?,
        rotation_duration: RotationDuration(set.req(ASSET_ROTATION_DURATION, u32_of)?),
        size: ByteCount(set.req(ASSET_SIZE, u32_of)?),
        asset_type: AssetType(set.req(ASSET_TYPE, u32_of)?),
        url: set.req(ASSET_URL, |s, f| text_of(s, f, AssetUrl::new))?,
        crex_id: CrexId(set.req(ASSET_CREX_ID, u32_of)?),
        angle_threshold: set.opt(ASSET_ANGLE, f32_of)?.map(AngleThreshold),
        distance_threshold: set.opt(ASSET_DISTANCE, f32_of)?.map(DistanceThreshold),
        size_threshold: set.opt(ASSET_SIZE_THRESHOLD, u16_of)?.map(SizeThreshold),
        extra: set
            .opt(Tag::ASSET_EXTRA, |s, f| {
                let inner = FieldSet::new(Context::AssetExtra, block_of(s, f)?);
                inner.opt(ASSET_EXTRA_VALUE, str_of)
            })?
            .flatten(),
    })
}

/// `CreateAsset` copies 20 bytes from the field without a length check; this
/// decoder requires exactly 20.
fn digest_of(set: &FieldSet<'_>, f: &Field) -> Result<AssetDigest> {
    let raw = bytes_of(set, f)?;
    <[u8; 20]>::try_from(raw).map(AssetDigest).map_err(|_| {
        set.invalid(
            f,
            InvalidReason::Length {
                expected: AssetDigest::LEN,
                found: raw.len(),
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    fn round_trip(resp: &Response) -> Vec<u8> {
        let key = MacKey::retail();
        let bytes = resp.encode(&key).unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert_eq!(&Response::decode(&frame).unwrap(), resp);
        bytes
    }

    fn sample_asset(id: u32) -> AssetRecord {
        let body = b"asset body";
        AssetRecord {
            asset_id: AssetId(id),
            expiration: Expiration(1),
            digest: AssetDigest::of(body),
            rotation_duration: RotationDuration(1),
            size: ByteCount(u32::try_from(body.len()).unwrap()),
            asset_type: AssetType(0),
            url: AssetUrl::new("http://127.0.0.1/assets/1").unwrap(),
            crex_id: CrexId(1),
            angle_threshold: None,
            distance_threshold: None,
            size_threshold: Some(SizeThreshold(95)),
            extra: None,
        }
    }

    fn sample_mao(media_type: u32) -> MaoRecord {
        MaoRecord {
            ie_id: IeId(1),
            ie_name: IeName::new("Video15_1").unwrap(),
            media_type: MediaType(media_type),
            rotation_type: RotationType(0),
            primary_match_list_size: 1,
            asset_ids: vec![AssetId(100)],
            angle_threshold: Some(AngleThreshold(0.5)),
            distance_threshold: None,
            size_threshold: None,
            adjustment: None,
        }
    }

    #[test]
    fn acknowledgement_layout() {
        let bytes = round_trip(&Response::Acknowledgement(Acknowledgement::Heartbeat));
        assert_eq!(&bytes[..7], &[0x05, 0xD7, 0, 0, 0, 23, 0x1E]);
    }

    #[test]
    fn open_session_round_trip() {
        round_trip(&Response::OpenSession(OpenSessionResponse {
            massive_player_id: MassivePlayerId(0x1122),
            session_id: SessionId(7),
            zones: vec![ZoneName::new("Lobby_PT").unwrap()],
        }));
    }

    #[test]
    fn locate_round_trip_without_signature() {
        let bytes = round_trip(&Response::LocateService(LocateServiceResponse {
            server_time: ServerTime(1),
            hosts: Service::ALL
                .map(|service| ServiceHost {
                    service,
                    host: HostName::new("192.168.0.2").unwrap(),
                })
                .to_vec(),
            ports: Service::ALL
                .map(|service| ServicePort {
                    service,
                    port: Port(80),
                })
                .to_vec(),
            auto_reporting_time: Some(Seconds(30)),
            server_options: None,
            statistic_period: Some(Minutes(5)),
            heartbeat_period: Some(Minutes(1)),
            logging_level: None,
            value_57: None,
        }));
        assert_eq!(bytes[1], 0xCA);
    }

    #[test]
    fn second_signature_is_counted() {
        let key = MacKey::retail();
        let mut fields = vec![Tag::SIGNATURE.0, 0x00, 0x14];
        fields.extend_from_slice(&[0; 20]);
        let bytes = FrameBuilder::new(&key)
            .build(MessageType::HeartbeatResponse, &fields)
            .unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert!(matches!(
            Response::decode(&frame),
            Err(DecodeError::FieldCount {
                required: 1,
                found: 2,
                ..
            })
        ));
    }

    #[test]
    fn locate_requires_exactly_one_time() {
        let key = MacKey::retail();
        let bytes = FrameBuilder::new(&key)
            .build(MessageType::LocateServiceResponse, &[0x0D, 0, 30])
            .unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert!(matches!(
            Response::decode(&frame),
            Err(DecodeError::FieldCount {
                required: 1,
                found: 0,
                ..
            })
        ));
    }

    #[test]
    fn enter_zone_round_trip() {
        round_trip(&Response::EnterZone(EnterZoneResponse {
            maos: vec![sample_mao(0x401)],
            orders: vec![Order {
                order_id: OrderId(5),
                frequency_cap: Some(FrequencyCap(3)),
                assets: vec![sample_asset(100)],
            }],
        }));
    }

    #[test]
    fn boundary_media_type_is_rejected() {
        let key = MacKey::retail();
        let bytes = Response::EnterZone(EnterZoneResponse {
            maos: vec![sample_mao(0x800)],
            orders: vec![],
        })
        .encode(&key)
        .unwrap();
        let frame = Frame::parse(&bytes).unwrap().verify(&key).unwrap();
        assert!(matches!(
            Response::decode(&frame),
            Err(DecodeError::Invalid {
                reason: InvalidReason::MediaType(0x800),
                ..
            })
        ));
    }

    #[test]
    fn asset_faults_follow_create_asset() {
        let mut asset = sample_asset(0);
        asset.rotation_duration = RotationDuration(0);
        assert_eq!(
            asset.faults(),
            vec![AssetFault::AssetIdZero, AssetFault::Value06OrExpirationZero]
        );
        asset.crex_id = CrexId(0);
        assert!(asset.faults().is_empty());
    }
}
