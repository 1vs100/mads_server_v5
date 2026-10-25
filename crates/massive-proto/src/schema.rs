//! Tag tables per message context.
//!
//! `layout` is the confidence in the tag and its value encoding, `meaning`
//! the confidence in what the value represents. Reader and writer addresses
//! are given where a table was taken from a single function.

#![allow(
    clippy::many_single_char_names,
    reason = "C, I and U mirror the confidence markers of the protocol book"
)]

use std::fmt;

use crate::confidence::Confidence::{self, Confirmed as C, Inferred as I, Unknown as U};
use crate::tag::Tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    U8,
    U16,
    U32,
    U64,
    F32,
    /// `u16` length and ASCII text, at most [`crate::wire::STR_MAX`] bytes.
    Str,
    /// `u16` length and raw bytes.
    Bytes,
    /// `u16` count and that many `u32` values.
    U32Array,
    /// `u32` length and a nested field region.
    Block(Context),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    pub tag: Tag,
    pub name: &'static str,
    pub kind: ValueKind,
    pub layout: Confidence,
    pub meaning: Confidence,
}

const fn f(
    tag: u8,
    name: &'static str,
    kind: ValueKind,
    layout: Confidence,
    meaning: Confidence,
) -> FieldSpec {
    FieldSpec {
        tag: Tag(tag),
        name,
        kind,
        layout,
        meaning,
    }
}

use ValueKind::{Block, Bytes, F32, Str, U8, U16, U32, U32Array, U64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Context {
    LocateServiceRequest,
    OpenSessionRequest,
    CloseSessionRequest,
    PlayerUpdateRequest,
    EnterZoneRequest,
    ImpressionUpdateRequest,
    HeartbeatRequest,
    StatisticsRequest,
    LocateServiceResponse,
    OpenSessionResponse,
    EnterZoneResponse,
    /// Responses to CloseSession, PlayerUpdate, ImpressionUpdate, Heartbeat and Statistics.
    Acknowledgement,
    PlayerBlock,
    ImpressionRecord,
    MaoRecord,
    OrderBlock,
    AssetRecord,
    AssetExtra,
}

const SEALED_TRAILER: [FieldSpec; 2] = [
    f(0x3B, "client_time", U64, C, I),
    f(0x1E, "signature", Bytes, C, C),
];

const PLAYERS: FieldSpec = f(0xE4, "player", Block(Context::PlayerBlock), C, C);
const MASSIVE_PLAYER_ID: FieldSpec = f(0x2A, "massive_player_id", U64, C, C);
const SESSION_ID: FieldSpec = f(0x2B, "massive_session_id", U64, C, C);
const SKU_NAME: FieldSpec = f(0x3D, "sku_name", Str, C, C);
const SKU_VERSION: FieldSpec = f(0x3E, "sku_version", Str, C, C);
const DEVICE_TOKEN: FieldSpec = f(0x1D, "device_token", Str, C, C);
const SIGNATURE: FieldSpec = SEALED_TRAILER[1];

macro_rules! table {
    ($($e:expr),* $(,)?) => {{
        const T: &[FieldSpec] = &[$($e),*];
        T
    }};
}

impl Context {
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one table per context")]
    pub fn fields(self) -> &'static [FieldSpec] {
        match self {
            // sub_824FEAF8
            Self::LocateServiceRequest => table![SKU_NAME, SKU_VERSION, DEVICE_TOKEN],
            // sub_82501818
            Self::OpenSessionRequest => table![
                SKU_NAME,
                SKU_VERSION,
                f(0x3C, "session_type", U8, C, C),
                f(0x18, "encryption_algorithm", U8, C, C),
                f(0x1B, "hash_algorithm", U8, C, C),
                f(0x44, "token", Str, C, I),
                DEVICE_TOKEN,
                f(0x16, "adclient_version", Str, C, C),
                PLAYERS,
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_824FE7F0
            Self::CloseSessionRequest => {
                table![
                    MASSIVE_PLAYER_ID,
                    SESSION_ID,
                    SEALED_TRAILER[0],
                    SEALED_TRAILER[1]
                ]
            }
            // sub_824FF310
            Self::PlayerUpdateRequest => table![
                MASSIVE_PLAYER_ID,
                SESSION_ID,
                PLAYERS,
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_82503798
            Self::EnterZoneRequest => table![
                f(0x47, "zone", Str, C, C),
                MASSIVE_PLAYER_ID,
                SESSION_ID,
                PLAYERS,
                f(0x10, "bandwidth_total_size", U32, C, C),
                f(0x11, "bandwidth_total_time", U32, C, C),
                f(0x12, "bandwidth_total_items", U16, C, C),
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_82502C50, sub_82503520
            Self::ImpressionUpdateRequest => table![
                SESSION_ID,
                MASSIVE_PLAYER_ID,
                f(0xE6, "impression", Block(Context::ImpressionRecord), C, C),
                PLAYERS,
                f(0x21, "counter_21", U16, C, U),
                f(0x29, "counter_29", U16, C, U),
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_825074E0
            Self::HeartbeatRequest => table![
                MASSIVE_PLAYER_ID,
                SESSION_ID,
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_82507800
            Self::StatisticsRequest => table![
                SESSION_ID,
                MASSIVE_PLAYER_ID,
                f(0x58, "cache_lookups", U16, C, C),
                f(0x59, "cache_hits", U16, C, C),
                f(0x5C, "cache_mismatches", U16, C, C),
                SEALED_TRAILER[0],
                SEALED_TRAILER[1],
            ],
            // sub_824FEBE8. Names from the v1.0.808.0 parser (sub_823D7AE8).
            Self::LocateServiceResponse => table![
                f(0x48, "zone_host", Str, C, C),
                f(0x49, "zone_port", U16, C, C),
                f(0x2D, "media_host", Str, C, C),
                f(0x4A, "media_port", U16, C, C),
                f(0x22, "impression_host", Str, C, C),
                f(0x4B, "impression_port", U16, C, C),
                f(0x0D, "auto_reporting_time_seconds", U16, C, C),
                f(0x3A, "server_configurable_options", U16, C, I),
                f(0x3B, "server_time", U64, C, I),
                f(0x3F, "statistic_period_minutes", U8, C, C),
                f(0x54, "heartbeat_period_minutes", U8, C, C),
                f(0x56, "logging_level", U8, C, C),
                f(0x57, "value_57", U8, C, U),
            ],
            // sub_825016D8
            Self::OpenSessionResponse => table![
                MASSIVE_PLAYER_ID,
                SESSION_ID,
                f(0x47, "zone", Str, C, I),
                SIGNATURE,
            ],
            // sub_824FE718, sub_824FF238, sub_825035D8, sub_82507408, sub_82507728
            Self::Acknowledgement => table![SIGNATURE],
            // sub_82504728
            Self::EnterZoneResponse => table![
                f(0x25, "mao_count", U16, C, C),
                f(0x55, "order_count", U16, C, C),
                f(0xDD, "mao", Block(Context::MaoRecord), C, C),
                f(0xE3, "order", Block(Context::OrderBlock), C, C),
                SIGNATURE,
            ],
            // sub_824FA218
            Self::PlayerBlock => {
                table![f(0x57, "slot", U8, C, C), f(0x41, "player_id", Bytes, C, U),]
            }
            // sub_824FA6F0
            Self::ImpressionRecord => table![f(0x3D, "ie_name", Str, C, C)],
            // sub_82503928. Names from the v1.0.808.0 parser (sub_823E7E68).
            Self::MaoRecord => table![
                f(0x08, "angle_threshold", F32, C, C),
                f(0x09, "distance_threshold", F32, C, C),
                f(0x0A, "size_threshold", U16, C, C),
                f(0x0B, "media_type", U32, C, C),
                f(0x23, "adjustment", F32, C, C),
                f(0x24, "asset_ids", U32Array, C, C),
                f(0x26, "ie_id", U32, C, C),
                f(0x27, "ie_name", Str, C, C),
                f(0x28, "rotation_type", U16, C, C),
                f(0x2E, "primary_match_list_size", U8, C, C),
            ],
            // sub_82504510
            Self::OrderBlock => table![
                f(0x01, "asset_count", U16, C, C),
                f(0x03, "frequency_cap", U32, C, C),
                f(0x13, "order_id", U32, C, C),
                f(0xDE, "asset", Block(Context::AssetRecord), C, C),
            ],
            // sub_82504200, jump table at 0x82084998. Names from the v1.0.808.0
            // parser (sub_823E84D0).
            Self::AssetRecord => table![
                f(0x02, "expiration_ms", U64, C, C),
                f(0x04, "digest", Bytes, C, C),
                f(0x05, "asset_id", U32, C, C),
                f(0x06, "rotation_duration_ms", U32, C, C),
                f(0x07, "size", U32, C, C),
                f(0x08, "angle_threshold", F32, C, C),
                f(0x09, "distance_threshold", F32, C, C),
                f(0x0A, "size_threshold", U16, C, C),
                f(0x0B, "asset_type", U32, C, C),
                f(0x0C, "url", Str, C, C),
                f(0x15, "crex_id", U32, C, C),
                f(0xE5, "extra", Block(Context::AssetExtra), C, U),
            ],
            // sub_82503DD8
            Self::AssetExtra => table![f(0x17, "value_17", Str, C, U)],
        }
    }

    #[must_use]
    pub fn spec(self, tag: Tag) -> Option<&'static FieldSpec> {
        self.fields().iter().find(|s| s.tag == tag)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LocateServiceRequest => "LocateServiceRequest",
            Self::OpenSessionRequest => "OpenSessionRequest",
            Self::CloseSessionRequest => "CloseSessionRequest",
            Self::PlayerUpdateRequest => "PlayerUpdateRequest",
            Self::EnterZoneRequest => "EnterZoneRequest",
            Self::ImpressionUpdateRequest => "ImpressionUpdateRequest",
            Self::HeartbeatRequest => "HeartbeatRequest",
            Self::StatisticsRequest => "StatisticsRequest",
            Self::LocateServiceResponse => "LocateServiceResponse",
            Self::OpenSessionResponse => "OpenSessionResponse",
            Self::EnterZoneResponse => "EnterZoneResponse",
            Self::Acknowledgement => "Acknowledgement",
            Self::PlayerBlock => "PlayerBlock",
            Self::ImpressionRecord => "ImpressionRecord",
            Self::MaoRecord => "MaoRecord",
            Self::OrderBlock => "OrderBlock",
            Self::AssetRecord => "AssetRecord",
            Self::AssetExtra => "AssetExtra",
        }
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Context; 18] = [
        Context::LocateServiceRequest,
        Context::OpenSessionRequest,
        Context::CloseSessionRequest,
        Context::PlayerUpdateRequest,
        Context::EnterZoneRequest,
        Context::ImpressionUpdateRequest,
        Context::HeartbeatRequest,
        Context::StatisticsRequest,
        Context::LocateServiceResponse,
        Context::OpenSessionResponse,
        Context::EnterZoneResponse,
        Context::Acknowledgement,
        Context::PlayerBlock,
        Context::ImpressionRecord,
        Context::MaoRecord,
        Context::OrderBlock,
        Context::AssetRecord,
        Context::AssetExtra,
    ];

    #[test]
    fn tags_are_unique_per_context() {
        for ctx in ALL {
            let fields = ctx.fields();
            for (i, a) in fields.iter().enumerate() {
                assert!(
                    fields[i + 1..].iter().all(|b| b.tag != a.tag),
                    "{ctx}: duplicate tag {}",
                    a.tag
                );
            }
        }
    }

    #[test]
    fn block_kinds_match_block_tags() {
        for ctx in ALL {
            for spec in ctx.fields() {
                assert_eq!(
                    matches!(spec.kind, ValueKind::Block(_)),
                    spec.tag.is_block(),
                    "{ctx}: {}",
                    spec.name
                );
            }
        }
    }
}
