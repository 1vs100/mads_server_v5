use std::fmt;

/// One-byte field tag.
///
/// Values below [`Tag::FIRST_BLOCK`] are followed by a value whose encoding
/// depends on the message context. Values at or above it are followed by a
/// `u32` length, so a reader that does not know them can skip them
/// (`sub_824F96C0`). An unknown tag below the threshold fails every retail
/// parser.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(pub u8);

impl Tag {
    pub const FIRST_BLOCK: u8 = 0xC8;

    pub const SIGNATURE: Self = Self(0x1E);
    pub const TIME: Self = Self(0x3B);
    pub const MASSIVE_PLAYER_ID: Self = Self(0x2A);
    pub const SESSION_ID: Self = Self(0x2B);
    pub const MAO_RECORD: Self = Self(0xDD);
    pub const ASSET_RECORD: Self = Self(0xDE);
    pub const ORDER_BLOCK: Self = Self(0xE3);
    pub const PLAYER_BLOCK: Self = Self(0xE4);
    pub const ASSET_EXTRA: Self = Self(0xE5);
    pub const IMPRESSION_RECORD: Self = Self(0xE6);

    #[must_use]
    pub const fn is_block(self) -> bool {
        self.0 >= Self::FIRST_BLOCK
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag(0x{:02X})", self.0)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}
