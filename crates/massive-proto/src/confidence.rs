use std::fmt;

/// Reverse engineering confidence of a protocol statement.
///
/// Matches the `[C]`, `[I]` and `[U]` markers of the protocol book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Read directly from the disassembly, the Lua source or a string in the binary.
    Confirmed,
    /// Indicated by control flow, naming or another client revision; not fully traced.
    Inferred,
    /// Identified but not decoded.
    Unknown,
}

impl Confidence {
    #[must_use]
    pub const fn marker(self) -> char {
        match self {
            Self::Confirmed => 'C',
            Self::Inferred => 'I',
            Self::Unknown => 'U',
        }
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.marker())
    }
}
