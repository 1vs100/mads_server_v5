//! HTTP framing used by the client (`sub_82506308`, format string at `0x82084BE0`).

use std::fmt::Write as _;

pub const CONTENT_TYPE: &str = "application/massive";
pub const USER_AGENT_PREFIX: &str = "Adclient Massive Inc./";
/// Appended to the user agent and sent in OpenSession field `0x16`.
pub const BUILD_STRING: &str = "XDK Test QFE 1v1009328.3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Every protocol request.
    Post,
    /// Asset downloads (request type `0x54`).
    Get,
}

impl Method {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Post => "POST",
            Self::Get => "GET",
        }
    }
}

/// Request head exactly as the client formats it, including the missing
/// space after each colon. Downloads use the same template.
#[must_use]
pub fn request_head(
    method: Method,
    target: &str,
    host: &str,
    port: u16,
    content_length: usize,
) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "{} {target} HTTP/1.1\r\nHost:{host}:{port}\r\nContent-Length:{content_length}\r\n\
         Content-Type:{CONTENT_TYPE}\r\nUser-Agent:{USER_AGENT_PREFIX}{BUILD_STRING}\r\n\r\n",
        method.as_str()
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_matches_template() {
        assert_eq!(
            request_head(Method::Post, "/adsrv/5/openSession", "10.0.0.1", 80, 12),
            "POST /adsrv/5/openSession HTTP/1.1\r\nHost:10.0.0.1:80\r\nContent-Length:12\r\n\
             Content-Type:application/massive\r\nUser-Agent:Adclient Massive Inc./XDK Test QFE 1v1009328.3\r\n\r\n"
        );
    }
}
