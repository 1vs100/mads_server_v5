//! Minimal HTTP/1.1 over a byte stream.
//!
//! Written by hand rather than with a framework so the raw bytes of every
//! exchange are available for capture and the response bytes, including
//! header case, are under exact control.

use std::fmt;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_HEAD: usize = 16 * 1024;
pub const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_HEADERS: usize = 32;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("message head exceeds {limit} bytes")]
    HeadTooLarge { limit: usize },
    #[error("malformed message head: {source}")]
    Malformed { source: httparse::Error },
    #[error("invalid Content-Length {value:?}")]
    ContentLength { value: String },
    #[error("body of {len} bytes exceeds {limit} bytes")]
    BodyTooLarge { len: usize, limit: usize },
    #[error("unsupported Transfer-Encoding {value:?}")]
    TransferEncoding { value: String },
    #[error("connection closed after {received} of {expected} bytes")]
    Truncated { expected: usize, received: usize },
    #[error("connection closed inside the message head after {received} bytes")]
    IncompleteHead { received: usize },
}

#[derive(Debug, Clone)]
pub struct Header {
    pub name: String,
    pub value: Vec<u8>,
}

/// Request target as sent: `/path` or `http://host/path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetForm {
    Origin,
    Absolute,
}

impl fmt::Display for TargetForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Origin => "origin",
            Self::Absolute => "absolute",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub minor_version: u8,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
    /// Head and body as received.
    pub raw: Vec<u8>,
}

impl Request {
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&[u8]> {
        header(&self.headers, name)
    }

    #[must_use]
    pub fn target_form(&self) -> TargetForm {
        if self.target.starts_with("http://") {
            TargetForm::Absolute
        } else {
            TargetForm::Origin
        }
    }

    /// Path for routing, with scheme, authority and query removed.
    #[must_use]
    pub fn path(&self) -> &str {
        let path = crate::content::url_path(&self.target);
        path.split_once('?').map_or(path, |(p, _)| p)
    }
}

fn header<'a>(headers: &'a [Header], name: &str) -> Option<&'a [u8]> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_slice())
}

fn content_length(headers: &[Header]) -> Result<Option<usize>, HttpError> {
    if let Some(te) = header(headers, "transfer-encoding") {
        return Err(HttpError::TransferEncoding {
            value: String::from_utf8_lossy(te).into_owned(),
        });
    }
    header(headers, "content-length")
        .map(|v| {
            let text = String::from_utf8_lossy(v);
            text.trim()
                .parse::<usize>()
                .map_err(|_| HttpError::ContentLength {
                    value: text.into_owned(),
                })
        })
        .transpose()
}

fn convert_headers(parsed: &[httparse::Header<'_>]) -> Vec<Header> {
    parsed
        .iter()
        .map(|h| Header {
            name: h.name.to_owned(),
            value: h.value.to_vec(),
        })
        .collect()
}

/// Reads until `buf` holds a complete head; returns its length, or `None`
/// when the peer closed before sending anything.
async fn fill_head<R, F>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    mut parse: F,
) -> Result<Option<usize>, HttpError>
where
    R: AsyncRead + Unpin,
    F: FnMut(&[u8]) -> Result<httparse::Status<usize>, httparse::Error>,
{
    loop {
        if !buf.is_empty() {
            match parse(buf).map_err(|source| HttpError::Malformed { source })? {
                httparse::Status::Complete(n) => return Ok(Some(n)),
                httparse::Status::Partial if buf.len() > MAX_HEAD => {
                    return Err(HttpError::HeadTooLarge { limit: MAX_HEAD });
                }
                httparse::Status::Partial => {}
            }
        }
        let mut chunk = [0u8; 4096];
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            return if buf.is_empty() {
                Ok(None)
            } else {
                Err(HttpError::IncompleteHead {
                    received: buf.len(),
                })
            };
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

async fn fill_exact<R: AsyncRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
    total: usize,
) -> Result<(), HttpError> {
    while buf.len() < total {
        let mut chunk = vec![0u8; (total - buf.len()).min(64 * 1024)];
        let n = reader.read(&mut chunk).await?;
        if n == 0 {
            return Err(HttpError::Truncated {
                expected: total,
                received: buf.len(),
            });
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(())
}

/// Reads one request. `buf` carries bytes received beyond the previous
/// request on the same connection. Returns `None` on a clean close.
pub async fn read_request<R: AsyncRead + Unpin>(
    reader: &mut R,
    buf: &mut Vec<u8>,
) -> Result<Option<Request>, HttpError> {
    let Some(head_len) = fill_head(reader, buf, |b| {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        httparse::Request::new(&mut headers).parse(b)
    })
    .await?
    else {
        return Ok(None);
    };
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut parsed = httparse::Request::new(&mut headers);
    parsed
        .parse(buf)
        .map_err(|source| HttpError::Malformed { source })?;
    let method = parsed.method.unwrap_or_default().to_owned();
    let target = parsed.path.unwrap_or_default().to_owned();
    let minor_version = parsed.version.unwrap_or_default();
    let headers = convert_headers(parsed.headers);
    let body_len = content_length(&headers)?.unwrap_or(0);
    if body_len > MAX_BODY {
        return Err(HttpError::BodyTooLarge {
            len: body_len,
            limit: MAX_BODY,
        });
    }
    fill_exact(reader, buf, head_len + body_len).await?;
    let raw: Vec<u8> = buf.drain(..head_len + body_len).collect();
    let body = raw[head_len..].to_vec();
    Ok(Some(Request {
        method,
        target,
        minor_version,
        headers,
        body,
        raw,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    BadRequest,
    NotFound,
    MethodNotAllowed,
    InternalServerError,
}

impl Status {
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Ok => 200,
            Self::BadRequest => 400,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::InternalServerError => 500,
        }
    }

    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::InternalServerError => "Internal Server Error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: Status,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    #[must_use]
    pub fn new(status: Status, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type,
            body,
        }
    }

    #[must_use]
    pub fn empty(status: Status) -> Self {
        Self::new(status, "text/plain", Vec::new())
    }

    /// Serialized response. Header names use the conventional mixed case.
    #[must_use]
    pub fn to_bytes(&self, close: bool) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n",
            self.status.code(),
            self.status.reason(),
            self.content_type,
            self.body.len()
        );
        if close {
            out.push_str("Connection: close\r\n");
        }
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// A response as read by the probe client.
#[derive(Debug, Clone)]
pub struct ReceivedResponse {
    pub code: u16,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

/// Writes `request` and reads one response with a `Content-Length` body.
pub async fn exchange<S>(stream: &mut S, request: &[u8]) -> Result<ReceivedResponse, HttpError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream.write_all(request).await?;
    stream.flush().await?;
    let mut buf = Vec::new();
    let head_len = fill_head(stream, &mut buf, |b| {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        httparse::Response::new(&mut headers).parse(b)
    })
    .await?
    .ok_or(HttpError::IncompleteHead { received: 0 })?;
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut parsed = httparse::Response::new(&mut headers);
    parsed
        .parse(&buf)
        .map_err(|source| HttpError::Malformed { source })?;
    let code = parsed.code.unwrap_or_default();
    let headers = convert_headers(parsed.headers);
    let body_len = content_length(&headers)?.ok_or(HttpError::ContentLength {
        value: String::new(),
    })?;
    fill_exact(stream, &mut buf, head_len + body_len).await?;
    Ok(ReceivedResponse {
        code,
        headers,
        body: buf[head_len..head_len + body_len].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_retail_head_without_spaces() {
        let head = massive_proto::transport::request_head(
            massive_proto::transport::Method::Post,
            "/adsrv/5/locateService",
            "10.0.0.1",
            80,
            3,
        );
        let mut input = head.into_bytes();
        input.extend_from_slice(b"abcGET /next");
        let mut reader = input.as_slice();
        let mut buf = Vec::new();
        let req = read_request(&mut reader, &mut buf).await.unwrap().unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path(), "/adsrv/5/locateService");
        assert_eq!(req.body, b"abc");
        assert_eq!(req.header("host"), Some(&b"10.0.0.1:80"[..]));
        assert_eq!(buf, b"GET /next");
    }

    #[tokio::test]
    async fn clean_close_yields_none() {
        let mut reader: &[u8] = &[];
        let mut buf = Vec::new();
        assert!(read_request(&mut reader, &mut buf).await.unwrap().is_none());
    }

    #[test]
    fn absolute_target_routes_by_path() {
        let req = Request {
            method: "GET".into(),
            target: "http://10.0.0.1:80/assets/7?x=1".into(),
            minor_version: 1,
            headers: vec![],
            body: vec![],
            raw: vec![],
        };
        assert_eq!(req.target_form(), TargetForm::Absolute);
        assert_eq!(req.path(), "/assets/7");
    }
}
