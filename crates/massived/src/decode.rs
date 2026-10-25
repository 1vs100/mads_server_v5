//! Offline decoding of messages and captures.

use std::io::{self, Write};
use std::path::PathBuf;

use massive_proto::crypto::MacKey;
use massive_proto::field::{Hex, Tree};
use massive_proto::frame::{Frame, HEADER_LEN};
use massive_proto::{Request, Response};
use thiserror::Error;

use crate::capture::{self, CaptureError, Direction};

#[derive(Debug, Clone)]
pub enum Input {
    /// One message or HTTP exchange as hex; whitespace is ignored.
    Hex(String),
    /// Raw bytes of one message or HTTP exchange.
    File(PathBuf),
    /// A capture written by the server.
    Capture(PathBuf),
}

#[derive(Debug, Error)]
pub enum DecodeCommandError {
    #[error("input is not hex: {source}")]
    Hex { source: hex::FromHexError },
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Output(#[from] io::Error),
}

pub fn run(input: &Input, key: &MacKey, out: &mut impl Write) -> Result<(), DecodeCommandError> {
    match input {
        Input::Hex(text) => {
            let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let bytes =
                hex::decode(cleaned).map_err(|source| DecodeCommandError::Hex { source })?;
            describe(&bytes, key, out)?;
        }
        Input::File(path) => {
            let bytes = std::fs::read(path).map_err(|source| DecodeCommandError::Read {
                path: path.clone(),
                source,
            })?;
            describe(&bytes, key, out)?;
        }
        Input::Capture(path) => {
            for (record, bytes) in capture::read(path)? {
                let arrow = match record.direction {
                    Direction::In => "client -> server",
                    Direction::Out => "server -> client",
                };
                writeln!(
                    out,
                    "=== {} ms, connection {}, {} {}",
                    record.time_ms, record.connection, record.peer, arrow
                )?;
                describe(&bytes, key, out)?;
                writeln!(out)?;
            }
        }
    }
    Ok(())
}

/// Splits an HTTP message into head and body. Returns the input as body
/// when it does not look like HTTP.
fn split_http(bytes: &[u8]) -> (Option<&[u8]>, &[u8]) {
    let looks_http = [&b"POST "[..], b"GET ", b"HTTP/"]
        .iter()
        .any(|p| bytes.starts_with(p));
    if !looks_http {
        return (None, bytes);
    }
    match bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(i) => (Some(&bytes[..i]), &bytes[i + 4..]),
        None => (Some(bytes), &[]),
    }
}

fn describe(bytes: &[u8], key: &MacKey, out: &mut impl Write) -> io::Result<()> {
    let (head, body) = split_http(bytes);
    if let Some(head) = head {
        for line in String::from_utf8_lossy(head).lines() {
            writeln!(out, "http  {line}")?;
        }
    }
    if body.is_empty() {
        return writeln!(out, "(no body)");
    }
    let frame = match Frame::parse(body) {
        Ok(frame) => frame,
        Err(e) => {
            writeln!(out, "frame error: {e}")?;
            return writeln!(out, "bytes {}", Hex(body));
        }
    };
    let type_name = frame.message_type().map_or("unknown type", |t| t.name());
    writeln!(
        out,
        "frame version {} type 0x{:02X} ({type_name}) length {}",
        frame.version(),
        frame.type_byte(),
        body.len() - HEADER_LEN
    )?;
    match (frame.signature(), frame.expected_mac(key)) {
        (Some(sig), Some(expected)) if sig.mac == expected => {
            writeln!(out, "mac   valid ({})", sig.mac)?;
        }
        (Some(sig), Some(expected)) => {
            writeln!(out, "mac   INVALID: found {}, expected {expected}", sig.mac)?;
        }
        _ => writeln!(out, "mac   none")?,
    }
    if let Some(partial) = frame.fields_partial() {
        write!(out, "{}", Tree(&partial.fields))?;
        if let Some(e) = partial.error {
            writeln!(out, "field error: {e}")?;
            return Ok(());
        }
    }
    let frame = frame.waive_verification();
    match frame.message_type() {
        Some(t) if t.is_request() => match Request::decode(&frame) {
            Ok(r) => writeln!(out, "{r:#?}"),
            Err(e) => writeln!(out, "decode error: {e}"),
        },
        Some(_) => match Response::decode(&frame) {
            Ok(r) => writeln!(out, "{r:#?}"),
            Err(e) => writeln!(out, "rejected by client rules: {e}"),
        },
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use massive_proto::response::Acknowledgement;

    #[test]
    fn describes_http_wrapped_response() {
        let key = MacKey::retail();
        let body = Response::Acknowledgement(Acknowledgement::Heartbeat)
            .encode(&key)
            .unwrap();
        let mut message = b"HTTP/1.1 200 OK\r\nContent-Length: 29\r\n\r\n".to_vec();
        message.extend_from_slice(&body);
        let mut out = Vec::new();
        run(&Input::Hex(hex::encode(&message)), &key, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("http  HTTP/1.1 200 OK"), "{text}");
        assert!(text.contains("type 0xD7 (HeartbeatResponse)"), "{text}");
        assert!(text.contains("mac   valid"), "{text}");
        assert!(text.contains("Heartbeat"), "{text}");
    }
}
