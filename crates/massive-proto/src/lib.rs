//! Codec for the Massive Inc. ad client protocol, revision 5, as linked into
//! *1 vs 100* (Xbox 360, build v2.0.1149.0.02, title 584A07D1).
//!
//! The crate has no I/O. Layers, from the bottom:
//!
//! - [`wire`]: big-endian primitives with offset-tracking errors.
//! - [`schema`] and [`field`]: tag tables per message context and a generic
//!   decoder that turns any message into a field tree. Every schema entry
//!   carries a [`Confidence`] for its layout and for its meaning.
//! - [`frame`]: the `05 | type | L | stream` envelope and its HMAC-SHA1.
//! - [`request`] and [`response`]: typed messages. Response decoders apply the
//!   acceptance rules of the retail client parsers.
//!
//! The reverse engineering status lives in the protocol book
//! (`massive-book/src/status.md`); addresses in the documentation refer to
//! the retail XEX.

#![forbid(unsafe_code)]

pub mod confidence;
pub mod crypto;
pub mod field;
pub mod frame;
pub mod ids;
pub mod request;
pub mod response;
pub mod schema;
pub mod tag;
pub mod transport;
pub mod wire;

mod fieldset;

pub use confidence::Confidence;
pub use field::{Field, Value};
pub use fieldset::{DecodeError, InvalidReason};
pub use frame::{Frame, MessageType, ProtocolVersion, Unverified, Verified};
pub use request::Request;
pub use response::Response;
pub use tag::Tag;
