//! Message envelope, both directions.
//!
//! ```text
//! version | type | u32 L | stream (L bytes)
//! stream  = fields [| 1E | 00 14 | MAC]
//! MAC     = HMAC-SHA1(key, type | L | fields | 1E)
//! ```
//!
//! `L` counts every byte after it, the signature trailer included. Requests
//! are sealed by `sub_824FA478`; every response parser starts with
//! `sub_824FA0B8`, which consumes the version byte and requires `0x05`.

use std::fmt;
use std::marker::PhantomData;

use thiserror::Error;

use crate::crypto::{Mac, MacKey};
use crate::field::{self, Field, FieldError, PartialFields};
use crate::schema::Context;
use crate::tag::Tag;
use crate::wire::{EncodeError, Reader, Truncated};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolVersion(pub u8);

impl ProtocolVersion {
    /// Revision 5 of the protocol, matching the `/adsrv/5/` paths.
    pub const RETAIL: Self = Self(5);
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageType {
    LocateServiceRequest = 0xC9,
    LocateServiceResponse = 0xCA,
    OpenSessionRequest = 0xCB,
    OpenSessionResponse = 0xCC,
    EnterZoneRequest = 0xCD,
    EnterZoneResponse = 0xCE,
    CloseSessionRequest = 0xD1,
    CloseSessionResponse = 0xD2,
    ImpressionUpdateRequest = 0xD3,
    ImpressionUpdateResponse = 0xD4,
    HeartbeatRequest = 0xD6,
    HeartbeatResponse = 0xD7,
    PlayerUpdateRequest = 0xD8,
    PlayerUpdateResponse = 0xD9,
    StatisticsRequest = 0xDA,
    StatisticsResponse = 0xDB,
}

impl MessageType {
    pub const ALL: [Self; 16] = [
        Self::LocateServiceRequest,
        Self::LocateServiceResponse,
        Self::OpenSessionRequest,
        Self::OpenSessionResponse,
        Self::EnterZoneRequest,
        Self::EnterZoneResponse,
        Self::CloseSessionRequest,
        Self::CloseSessionResponse,
        Self::ImpressionUpdateRequest,
        Self::ImpressionUpdateResponse,
        Self::HeartbeatRequest,
        Self::HeartbeatResponse,
        Self::PlayerUpdateRequest,
        Self::PlayerUpdateResponse,
        Self::StatisticsRequest,
        Self::StatisticsResponse,
    ];

    #[must_use]
    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|t| *t as u8 == b)
    }

    #[must_use]
    pub const fn byte(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn is_request(self) -> bool {
        matches!(
            self,
            Self::LocateServiceRequest
                | Self::OpenSessionRequest
                | Self::EnterZoneRequest
                | Self::CloseSessionRequest
                | Self::ImpressionUpdateRequest
                | Self::HeartbeatRequest
                | Self::PlayerUpdateRequest
                | Self::StatisticsRequest
        )
    }

    /// Locate is the only exchange without a MAC in either direction.
    #[must_use]
    pub const fn is_signed(self) -> bool {
        !matches!(
            self,
            Self::LocateServiceRequest | Self::LocateServiceResponse
        )
    }

    /// Each response type byte is its request type byte plus one.
    #[must_use]
    pub fn response(self) -> Option<Self> {
        if self.is_request() {
            Self::from_byte(self.byte() + 1)
        } else {
            None
        }
    }

    /// HTTP path of a request (vtable `+0x08` of each request object).
    #[must_use]
    pub const fn path(self) -> Option<&'static str> {
        Some(match self {
            Self::LocateServiceRequest => "/adsrv/5/locateService",
            Self::OpenSessionRequest => "/adsrv/5/openSession",
            Self::CloseSessionRequest => "/adsrv/5/closeSession",
            Self::PlayerUpdateRequest => "/adsrv/5/updatePlayers",
            Self::EnterZoneRequest => "/adsrv/5/enterZone",
            Self::ImpressionUpdateRequest => "/impsrv/5/activity",
            Self::HeartbeatRequest => "/impsrv/5/heartbeat",
            Self::StatisticsRequest => "/impsrv/5/stats",
            _ => return None,
        })
    }

    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.path() == Some(path))
    }

    #[must_use]
    pub const fn context(self) -> Context {
        match self {
            Self::LocateServiceRequest => Context::LocateServiceRequest,
            Self::OpenSessionRequest => Context::OpenSessionRequest,
            Self::CloseSessionRequest => Context::CloseSessionRequest,
            Self::PlayerUpdateRequest => Context::PlayerUpdateRequest,
            Self::EnterZoneRequest => Context::EnterZoneRequest,
            Self::ImpressionUpdateRequest => Context::ImpressionUpdateRequest,
            Self::HeartbeatRequest => Context::HeartbeatRequest,
            Self::StatisticsRequest => Context::StatisticsRequest,
            Self::LocateServiceResponse => Context::LocateServiceResponse,
            Self::OpenSessionResponse => Context::OpenSessionResponse,
            Self::EnterZoneResponse => Context::EnterZoneResponse,
            Self::CloseSessionResponse
            | Self::ImpressionUpdateResponse
            | Self::HeartbeatResponse
            | Self::PlayerUpdateResponse
            | Self::StatisticsResponse => Context::Acknowledgement,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LocateServiceRequest => "LocateServiceRequest",
            Self::LocateServiceResponse => "LocateServiceResponse",
            Self::OpenSessionRequest => "OpenSessionRequest",
            Self::OpenSessionResponse => "OpenSessionResponse",
            Self::EnterZoneRequest => "EnterZoneRequest",
            Self::EnterZoneResponse => "EnterZoneResponse",
            Self::CloseSessionRequest => "CloseSessionRequest",
            Self::CloseSessionResponse => "CloseSessionResponse",
            Self::ImpressionUpdateRequest => "ImpressionUpdateRequest",
            Self::ImpressionUpdateResponse => "ImpressionUpdateResponse",
            Self::HeartbeatRequest => "HeartbeatRequest",
            Self::HeartbeatResponse => "HeartbeatResponse",
            Self::PlayerUpdateRequest => "PlayerUpdateRequest",
            Self::PlayerUpdateResponse => "PlayerUpdateResponse",
            Self::StatisticsRequest => "StatisticsRequest",
            Self::StatisticsResponse => "StatisticsResponse",
        }
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Bytes before the stream: version, type and `L`.
pub const HEADER_LEN: usize = 6;
/// `1E | 00 14 | MAC`.
pub const TRAILER_LEN: usize = 1 + 2 + Mac::LEN;
const SIGNATURE_LEN_FIELD: [u8; 2] = [0x00, 0x14];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FrameError {
    #[error(transparent)]
    Truncated(#[from] Truncated),
    #[error("declared length {declared} but {available} bytes follow the header")]
    LengthMismatch { declared: u32, available: usize },
    #[error("{message_type} must end with a signature trailer")]
    MissingSignature { message_type: MessageType },
    #[error("signature length field is {found}, expected 20")]
    SignatureLength { found: u16 },
}

/// Typestate marker: the MAC has not been checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unverified;

/// Typestate marker: the MAC matched, the message type carries no MAC, or
/// the caller explicitly waived the check with [`Frame::waive_verification`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    /// Absolute offset of the `0x1E` tag byte.
    pub offset: usize,
    pub mac: Mac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<S = Unverified> {
    bytes: Vec<u8>,
    signature: Option<Signature>,
    state: PhantomData<S>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacMismatch {
    pub frame: Frame<Unverified>,
    pub expected: Mac,
    pub found: Mac,
}

impl fmt::Display for MacMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MAC mismatch: expected {}, found {}",
            self.expected, self.found
        )
    }
}

impl std::error::Error for MacMismatch {}

impl Frame<Unverified> {
    /// Parses one message that occupies all of `bytes`.
    ///
    /// The signature is located by the message type. For unknown types it is
    /// detected by its trailer shape. The version byte is not checked here;
    /// see [`Frame::version`].
    pub fn parse(bytes: &[u8]) -> Result<Self, FrameError> {
        let mut r = Reader::new(bytes, 0);
        r.u8()?;
        let type_byte = r.u8()?;
        let declared = r.u32()?;
        let available = r.remaining().len();
        if usize::try_from(declared).ok() != Some(available) {
            return Err(FrameError::LengthMismatch {
                declared,
                available,
            });
        }
        let has_trailer_shape =
            available >= TRAILER_LEN && bytes[bytes.len() - TRAILER_LEN] == Tag::SIGNATURE.0;
        let signed = match MessageType::from_byte(type_byte) {
            Some(t) if t.is_signed() => {
                if !has_trailer_shape {
                    return Err(FrameError::MissingSignature { message_type: t });
                }
                true
            }
            Some(_) => false,
            None => has_trailer_shape,
        };
        let signature = if signed {
            let offset = bytes.len() - TRAILER_LEN;
            let len = u16::from_be_bytes([bytes[offset + 1], bytes[offset + 2]]);
            if usize::from(len) != Mac::LEN {
                return Err(FrameError::SignatureLength { found: len });
            }
            let mut mac = [0; Mac::LEN];
            mac.copy_from_slice(&bytes[offset + 3..]);
            Some(Signature {
                offset,
                mac: Mac(mac),
            })
        } else {
            None
        };
        Ok(Self {
            bytes: bytes.to_vec(),
            signature,
            state: PhantomData,
        })
    }

    /// MAC the frame should carry under `key`, if it is signed.
    #[must_use]
    pub fn expected_mac(&self, key: &MacKey) -> Option<Mac> {
        self.mac_input().map(|data| key.sign(data))
    }

    pub fn verify(self, key: &MacKey) -> Result<Frame<Verified>, Box<MacMismatch>> {
        match (self.signature, self.expected_mac(key)) {
            (Some(sig), Some(expected)) if sig.mac != expected => Err(Box::new(MacMismatch {
                frame: self,
                expected,
                found: sig.mac,
            })),
            _ => Ok(self.into_state()),
        }
    }

    /// Accepts the frame without checking the MAC. For diagnosis of traffic
    /// signed with an unknown key.
    #[must_use]
    pub fn waive_verification(self) -> Frame<Verified> {
        self.into_state()
    }
}

impl<S> Frame<S> {
    fn into_state<T>(self) -> Frame<T> {
        Frame {
            bytes: self.bytes,
            signature: self.signature,
            state: PhantomData,
        }
    }

    #[must_use]
    pub fn version(&self) -> ProtocolVersion {
        ProtocolVersion(self.bytes[0])
    }

    #[must_use]
    pub fn type_byte(&self) -> u8 {
        self.bytes[1]
    }

    #[must_use]
    pub fn message_type(&self) -> Option<MessageType> {
        MessageType::from_byte(self.type_byte())
    }

    #[must_use]
    pub fn signature(&self) -> Option<Signature> {
        self.signature
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Field region: the stream without the signature trailer.
    #[must_use]
    pub fn fields_bytes(&self) -> &[u8] {
        let end = self.signature.map_or(self.bytes.len(), |s| s.offset);
        &self.bytes[HEADER_LEN..end]
    }

    /// Bytes covered by the MAC: type through the `0x1E` tag.
    #[must_use]
    pub fn mac_input(&self) -> Option<&[u8]> {
        self.signature.map(|s| &self.bytes[1..=s.offset])
    }

    /// Decodes the field region with the schema of the message type.
    #[must_use]
    pub fn fields_partial(&self) -> Option<PartialFields> {
        let context = self.message_type()?.context();
        Some(field::decode_partial(
            Reader::new(self.fields_bytes(), HEADER_LEN),
            context,
        ))
    }

    pub(crate) fn fields_in(&self, context: Context) -> Result<Vec<Field>, FieldError> {
        field::decode(Reader::new(self.fields_bytes(), HEADER_LEN), context)
    }
}

/// Builds a frame. The key signs every message type except locate, which
/// carries no MAC in either direction.
#[derive(Debug, Clone)]
pub struct FrameBuilder<'k> {
    version: ProtocolVersion,
    key: &'k MacKey,
}

impl<'k> FrameBuilder<'k> {
    #[must_use]
    pub fn new(key: &'k MacKey) -> Self {
        Self {
            version: ProtocolVersion::RETAIL,
            key,
        }
    }

    #[must_use]
    pub fn version(mut self, version: ProtocolVersion) -> Self {
        self.version = version;
        self
    }

    pub fn build(&self, message_type: MessageType, fields: &[u8]) -> Result<Vec<u8>, EncodeError> {
        let key = message_type.is_signed().then_some(self.key);
        self.build_raw(message_type.byte(), fields, key)
    }

    /// Any type byte, signed when `key` is given. For crafting test input.
    pub fn build_raw(
        &self,
        type_byte: u8,
        fields: &[u8],
        key: Option<&MacKey>,
    ) -> Result<Vec<u8>, EncodeError> {
        let trailer = if key.is_some() { TRAILER_LEN } else { 0 };
        let len = u32::try_from(fields.len() + trailer).map_err(|_| EncodeError::TooLong {
            len: fields.len() + trailer,
            max: u32::MAX as usize,
        })?;
        let mut out = Vec::with_capacity(HEADER_LEN + fields.len() + trailer);
        out.push(self.version.0);
        out.push(type_byte);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(fields);
        if let Some(key) = key {
            out.push(Tag::SIGNATURE.0);
            let mac = key.sign(&out[1..]);
            out.extend_from_slice(&SIGNATURE_LEN_FIELD);
            out.extend_from_slice(&mac.0);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_type_is_request_plus_one() {
        for t in MessageType::ALL.into_iter().filter(|t| t.is_request()) {
            let r = t.response().unwrap();
            assert_eq!(r.byte(), t.byte() + 1);
            assert!(!r.is_request());
        }
    }

    #[test]
    fn signed_round_trip_and_tamper() {
        let key = MacKey::retail();
        let bytes = FrameBuilder::new(&key)
            .build(MessageType::HeartbeatResponse, &[])
            .unwrap();
        assert_eq!(bytes.len(), HEADER_LEN + TRAILER_LEN);
        assert_eq!(&bytes[..6], &[0x05, 0xD7, 0, 0, 0, 23]);
        let frame = Frame::parse(&bytes).unwrap();
        assert_eq!(frame.mac_input().unwrap(), &bytes[1..7]);
        frame.clone().verify(&key).unwrap();
        let mut bad = bytes.clone();
        bad[5] ^= 1;
        assert!(matches!(
            Frame::parse(&bad),
            Err(FrameError::LengthMismatch { .. })
        ));
        let mut bad = bytes;
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(Frame::parse(&bad).unwrap().verify(&key).is_err());
    }

    #[test]
    fn signed_type_without_trailer_is_rejected() {
        let bytes = FrameBuilder::new(&MacKey::retail())
            .build_raw(MessageType::HeartbeatResponse.byte(), &[0x3D, 0, 0], None)
            .unwrap();
        assert_eq!(
            Frame::parse(&bytes),
            Err(FrameError::MissingSignature {
                message_type: MessageType::HeartbeatResponse
            })
        );
    }

    #[test]
    fn locate_is_unsigned() {
        let bytes = FrameBuilder::new(&MacKey::retail())
            .build(MessageType::LocateServiceRequest, &[0x3D, 0, 0])
            .unwrap();
        let frame = Frame::parse(&bytes).unwrap();
        assert!(frame.signature().is_none());
        assert_eq!(frame.fields_bytes(), &[0x3D, 0, 0]);
    }

    #[test]
    fn paths_map_back_to_requests() {
        for t in MessageType::ALL {
            if let Some(p) = t.path() {
                assert_eq!(MessageType::from_path(p), Some(t));
                assert!(t.is_request());
            }
        }
    }
}
