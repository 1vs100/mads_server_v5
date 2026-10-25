//! Field server for the Massive ad client protocol, revision 5.
//!
//! The server answers the retail client, logs every observation the reverse
//! engineering status asks for, and can capture raw exchanges. The `decode`
//! and `probe` commands analyze captures and exercise a server without a
//! console.

#![forbid(unsafe_code)]

pub mod capture;
pub mod config;
pub mod content;
pub mod decode;
pub mod handler;
pub mod http;
pub mod logging;
pub mod probe;
pub mod server;
pub mod session;
