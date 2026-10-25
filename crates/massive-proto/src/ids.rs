//! Domain value types.
//!
//! Names follow the reverse engineering notes. Where only the encoding is
//! known the type says so and offers no interpretation.

use std::fmt;

use thiserror::Error;

macro_rules! scalar {
    ($(#[$m:meta])* $name:ident($inner:ty)) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $inner);

        impl $name {
            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

macro_rules! text {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Accepts printable ASCII of at most [`crate::wire::STR_MAX`] bytes.
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidText> {
                let value = value.into();
                validate_text(&value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("text {value:?} is not printable ASCII of at most {max} bytes")]
pub struct InvalidText {
    pub value: String,
    pub max: usize,
}

fn validate_text(value: &str) -> Result<(), InvalidText> {
    let ok =
        value.len() <= crate::wire::STR_MAX && value.bytes().all(|b| (0x20..0x7F).contains(&b));
    if ok {
        Ok(())
    } else {
        Err(InvalidText {
            value: value.to_owned(),
            max: crate::wire::STR_MAX,
        })
    }
}

scalar!(
    /// Field `0x2B`, `qword_82A0EEA8`. The client clears it after CloseSession.
    SessionId(u64)
);
scalar!(
    /// Field `0x2A`, `qword_82A0EEB0`: the Massive player id the server assigns.
    MassivePlayerId(u64)
);
scalar!(
    /// Field `0x3B` of sealed requests (`sub_824FE140`). Unix time in ms in the
    /// v1.0.808.0 capture.
    ClientTime(u64)
);
scalar!(
    /// Field `0x3B` of the locate response, stored at request `+0x70`. Its use is unknown.
    ServerTime(u64)
);
scalar!(
    /// Field `0x26` of a MAO record.
    IeId(u32)
);
scalar!(
    /// Asset field `0x05`; the values MAO field `0x24` refers to.
    AssetId(u32)
);
scalar!(
    /// Order field `0x13`, first argument of `CreateOrder` (`0x8250B740`).
    OrderId(u32)
);
scalar!(
    /// Order field `0x03`, second argument of `CreateOrder`.
    FrequencyCap(u32)
);
scalar!(
    /// Asset field `0x0B`.
    AssetType(u32)
);
scalar!(
    /// Asset field `0x02`, Unix time in ms.
    Expiration(u64)
);
scalar!(
    /// Asset field `0x06`: display time in ms.
    RotationDuration(u32)
);
scalar!(
    /// Asset field `0x15`: creative id. Zero makes `CreateAsset` skip its checks.
    CrexId(u32)
);
scalar!(
    /// Asset field `0x07`: exact length the download must have.
    ByteCount(u32)
);
scalar!(
    /// Field `0x0A` of MAO and asset records. The client default is 95.
    SizeThreshold(u16)
);
scalar!(
    /// Player block field `0x57`.
    PlayerSlot(u8)
);
scalar!(
    /// TCP port.
    Port(u16)
);
scalar!(
    /// Minutes; the locate handler multiplies by 60000 ms.
    Minutes(u8)
);
scalar!(
    /// Seconds; the locate handler multiplies by 1000 ms.
    Seconds(u16)
);
scalar!(
    /// Locate field `0x3A`, server configurable options, ORed into client `+0x11C`.
    ServerOptions(u16)
);

text!(
    /// Field `0x3D`, the first `Initialize` argument.
    SkuName
);
text!(
    /// Field `0x3E`, the second `Initialize` argument.
    SkuVersion
);
text!(
    /// Field `0x47`.
    ZoneName
);
text!(
    /// Inventory element name: MAO field `0x27`, impression record field `0x3D`.
    IeName
);
text!(
    /// OpenSession field `0x16`.
    BuildString
);
text!(
    /// Host name or address in the locate response.
    HostName
);
text!(
    /// OpenSession field `0x44`. Equals [`crate::crypto::MacKey::RETAIL`] in this build.
    KeyString
);
text!(
    /// Asset field `0x0C`, a path such as `/<order>/<crex>/<file>`.
    AssetUrl
);

/// Angle threshold. Unit not determined.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AngleThreshold(pub f32);

/// Distance threshold. Unit not determined.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DistanceThreshold(pub f32);

/// 16-byte player identifier from player block field `0x41`. Meaning unknown.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(pub [u8; 16]);

impl fmt::Debug for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlayerId({})", crate::field::Hex(&self.0))
    }
}

/// MAO field `0x0B`. Selects the object class in `sub_82503928`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MediaType(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaClass {
    Texture,
    Video,
    Audio,
    Model,
}

impl MediaType {
    /// Class chosen by the compare chain. The values `0x400`, `0x800` and
    /// `0x1000` match no branch, no object is created and the response is
    /// rejected. The class names are confirmed for the dynamic creator path
    /// and inferred for the static path. The compares are signed (`cmpwi`).
    #[must_use]
    pub const fn class(self) -> Option<MediaClass> {
        #[expect(clippy::cast_possible_wrap, reason = "the client compares signed")]
        let signed = self.0 as i32;
        match signed {
            i32::MIN..=0x3FF => Some(MediaClass::Texture),
            0x401..=0x7FF => Some(MediaClass::Video),
            0x801..=0xFFF => Some(MediaClass::Audio),
            0x1001.. => Some(MediaClass::Model),
            _ => None,
        }
    }
}

/// MAO field `0x28`. The value `0x10` places the record in the dynamic
/// creator list; the meaning of other values is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RotationType(pub u16);

impl RotationType {
    pub const DYNAMIC_CREATOR: Self = Self(0x10);

    #[must_use]
    pub fn is_dynamic_creator(self) -> bool {
        self == Self::DYNAMIC_CREATOR
    }
}

/// The three services named by the locate response, by client list index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    /// Index 3, tags `0x48`/`0x49`.
    Zone,
    /// Index 4, tags `0x2D`/`0x4A`.
    Media,
    /// Index 5, tags `0x22`/`0x4B`.
    Impression,
}

impl Service {
    pub const ALL: [Self; 3] = [Self::Zone, Self::Media, Self::Impression];

    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::Zone => 3,
            Self::Media => 4,
            Self::Impression => 5,
        }
    }

    #[must_use]
    pub const fn host_tag(self) -> crate::Tag {
        crate::Tag(match self {
            Self::Zone => 0x48,
            Self::Media => 0x2D,
            Self::Impression => 0x22,
        })
    }

    #[must_use]
    pub const fn port_tag(self) -> crate::Tag {
        crate::Tag(match self {
            Self::Zone => 0x49,
            Self::Media => 0x4A,
            Self::Impression => 0x4B,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_boundaries_select_no_class() {
        for v in [0x400, 0x800, 0x1000] {
            assert_eq!(MediaType(v).class(), None, "{v:#x}");
        }
        assert_eq!(MediaType(0x3FF).class(), Some(MediaClass::Texture));
        assert_eq!(MediaType(0x401).class(), Some(MediaClass::Video));
        assert_eq!(MediaType(0x801).class(), Some(MediaClass::Audio));
        assert_eq!(MediaType(0x1001).class(), Some(MediaClass::Model));
        assert_eq!(MediaType(0x8000_0000).class(), Some(MediaClass::Texture));
    }

    #[test]
    fn text_rejects_control_bytes() {
        assert!(ZoneName::new("Lobby_PT").is_ok());
        assert!(ZoneName::new("a\0").is_err());
    }
}
