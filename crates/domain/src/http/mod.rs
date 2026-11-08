//! Minimal HTTP/1.x message handling: request-head parsing with hard limits,
//! response-head serialisation. Pure functions over bytes.

pub mod chunked;
pub mod range;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::fmt::Write as _;

/// Parser limits; everything beyond is rejected.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum size of request line + headers.
    pub max_head_bytes: usize,
    pub max_headers: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_head_bytes: 16 * 1024, max_headers: 100 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    HeadTooLarge,
    TooManyHeaders,
    Malformed(&'static str),
    UnsupportedVersion,
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HeadTooLarge => write!(f, "request head too large"),
            Self::TooManyHeaders => write!(f, "too many headers"),
            Self::Malformed(w) => write!(f, "malformed request: {w}"),
            Self::UnsupportedVersion => write!(f, "unsupported HTTP version"),
        }
    }
}

impl core::error::Error for HttpError {}

/// A parsed request line + headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHead {
    pub method: String,
    pub target: String,
    /// 0 for HTTP/1.0, 1 for HTTP/1.1.
    pub minor_version: u8,
    /// Header names lower-cased, values trimmed.
    pub headers: Vec<(String, String)>,
}

impl RequestHead {
    /// First value of a header (name must be lower-case).
    #[must_use]
    pub fn header(&self, lower_name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == lower_name).map(|(_, v)| v.as_str())
    }

    /// Host without port, lower-cased.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        let h = self.header("host")?;
        Some(
            h.rsplit_once(':')
                .map_or(h, |(name, port)| if port.bytes().all(|b| b.is_ascii_digit()) { name } else { h }),
        )
    }

    /// Whether the connection stays open after the response.
    #[must_use]
    pub fn keep_alive(&self) -> bool {
        let conn = self.header("connection").unwrap_or("");
        let has = |token: &str| conn.split(',').any(|t| t.trim().eq_ignore_ascii_case(token));
        if self.minor_version >= 1 { !has("close") } else { has("keep-alive") }
    }

    /// Request has a body (none is accepted).
    #[must_use]
    pub fn has_body(&self) -> bool {
        self.header("transfer-encoding").is_some() || self.header("content-length").is_some_and(|v| v.trim() != "0")
    }
}

/// Tries to parse a request head from the start of `buf`.
///
/// Returns `Ok(None)` if more bytes are needed, `Ok(Some((head, consumed)))`
/// once `\r\n\r\n` was seen.
///
/// # Errors
/// Limits exceeded or malformed head.
pub fn parse_request_head(buf: &[u8], limits: Limits) -> Result<Option<(RequestHead, usize)>, HttpError> {
    let Some(end) = find_head_end(buf) else {
        return if buf.len() > limits.max_head_bytes { Err(HttpError::HeadTooLarge) } else { Ok(None) };
    };
    if end > limits.max_head_bytes {
        return Err(HttpError::HeadTooLarge);
    }
    let text = core::str::from_utf8(&buf[..end - 4]).map_err(|_| HttpError::Malformed("non-UTF-8 head"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(HttpError::Malformed("empty"))?;
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HttpError::Malformed("request line"));
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(HttpError::Malformed("method"));
    }
    // Origin form only (`/path?query`): a full URL names a host of its own, never relayed.
    if !target.starts_with('/') || target.bytes().any(|b| b <= b' ' || b == 0x7F) {
        return Err(HttpError::Malformed("target"));
    }
    let minor_version = match version {
        "HTTP/1.1" => 1,
        "HTTP/1.0" => 0,
        _ => return Err(HttpError::UnsupportedVersion),
    };
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() == limits.max_headers {
            return Err(HttpError::TooManyHeaders);
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::Malformed("header line"))?;
        if name.is_empty() || name.bytes().any(|b| b <= b' ' || b == b':' || b >= 0x7F) {
            return Err(HttpError::Malformed("header name"));
        }
        if !value_ok(value) {
            return Err(HttpError::Malformed("header value"));
        }
        headers.push((name.to_ascii_lowercase(), String::from(value.trim())));
    }
    one_length(&headers)?;
    Ok(Some((RequestHead { method: String::from(method), target: String::from(target), minor_version, headers }, end)))
}

/// A header value holds no line break or other control character but a tab: one that does
/// would be a header (or a request) of its own once sent on.
fn value_ok(value: &str) -> bool {
    !value.bytes().any(|b| (b < b' ' && b != b'\t') || b == 0x7F)
}

/// One `Content-Length` (said again only with the same value), digits only: two that differ
/// (or a `+5`) are read differently by different servers, and a body one of them sees is the
/// next request to another.
fn one_length(headers: &[(String, String)]) -> Result<(), HttpError> {
    let mut lengths = headers.iter().filter(|(n, _)| n == "content-length").map(|(_, v)| v);
    let Some(first) = lengths.next() else { return Ok(()) };
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) {
        return Err(HttpError::Malformed("content-length"));
    }
    if lengths.any(|v| v != first) {
        return Err(HttpError::Malformed("content-length twice"));
    }
    Ok(())
}

/// Writes `name: value` lines; one that would break the head (a line break in it) is left out.
fn write_headers(s: &mut String, headers: &[(&str, &str)]) {
    for (n, v) in headers {
        if !n.contains(['\r', '\n']) && !v.contains(['\r', '\n']) {
            let _ = write!(s, "{n}: {v}\r\n");
        }
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

/// Serialises a response head with `Content-Length`.
#[must_use]
pub fn response_head(
    status: u16,
    reason: &str,
    headers: &[(&str, &str)],
    content_length: u64,
    keep_alive: bool,
) -> Vec<u8> {
    let mut s = String::with_capacity(128);
    let _ = write!(s, "HTTP/1.1 {status} {reason}\r\nContent-Length: {content_length}\r\n");
    write_headers(&mut s, headers);
    s.push_str(if keep_alive { "Connection: keep-alive\r\n\r\n" } else { "Connection: close\r\n\r\n" });
    s.into_bytes()
}

/// Head of a response whose body runs until the connection closes (a stream,
/// e.g. server-sent events): no `Content-Length`, `Connection: close`.
#[must_use]
pub fn stream_response_head(status: u16, reason: &str, headers: &[(&str, &str)]) -> Vec<u8> {
    let mut s = String::with_capacity(128);
    let _ = write!(s, "HTTP/1.1 {status} {reason}\r\n");
    write_headers(&mut s, headers);
    s.push_str("Connection: close\r\n\r\n");
    s.into_bytes()
}

/// Response headers stored with an object and sent again on hits.
pub const STORED_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-encoding",
    "cache-control",
    "expires",
    "last-modified",
    "etag",
    "x-content-crc",
    "content-md5",
];

/// Stored headers that describe the whole body; not sent with partial (206) responses.
pub const WHOLE_BODY_HEADERS: &[&str] = &["x-content-crc", "content-md5"];

/// Standard reason phrase (`"Unknown"` for unlisted codes; clients ignore it).
#[must_use]
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown",
    }
}

/// Response headers relayed from an origin to clients (and kept in record
/// metadata). Hop-by-hop and framing headers are deliberately absent: lanstash
/// sets its own `Content-Length` and `Connection`.
pub const RELAYED_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-encoding",
    "content-range",
    "cache-control",
    "expires",
    "last-modified",
    "etag",
    "location",
    "x-content-crc",
    "content-md5",
];

/// A parsed response status line + headers (from an upstream server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseHeadIn {
    pub status: u16,
    pub minor_version: u8,
    /// Header names lower-cased, values trimmed.
    pub headers: Vec<(String, String)>,
}

impl ResponseHeadIn {
    #[must_use]
    pub fn header(&self, lower_name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n == lower_name).map(|(_, v)| v.as_str())
    }

    /// The headers to relay to clients ([`RELAYED_RESPONSE_HEADERS`]).
    #[must_use]
    pub fn relayed(&self) -> Vec<(&str, &str)> {
        self.headers
            .iter()
            .filter(|(n, _)| RELAYED_RESPONSE_HEADERS.contains(&n.as_str()))
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect()
    }

    /// Declared body length (ignored when chunked).
    #[must_use]
    pub fn content_length(&self) -> Option<u64> {
        if self.is_chunked() {
            return None;
        }
        self.header("content-length").and_then(|v| v.trim().parse().ok())
    }

    #[must_use]
    pub fn is_chunked(&self) -> bool {
        self.header("transfer-encoding").is_some_and(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case("chunked")))
    }

    /// Whether the server keeps the connection open.
    #[must_use]
    pub fn keep_alive(&self) -> bool {
        let conn = self.header("connection").unwrap_or("");
        let has = |t: &str| conn.split(',').any(|x| x.trim().eq_ignore_ascii_case(t));
        if self.minor_version >= 1 { !has("close") } else { has("keep-alive") }
    }

    /// Responses that never have a body.
    #[must_use]
    pub fn bodyless(&self) -> bool {
        self.status == 204 || self.status == 304 || (100..200).contains(&self.status)
    }
}

/// Parses a response head from the start of `buf` (`Ok(None)`: need more bytes).
///
/// # Errors
/// Limits exceeded or malformed head.
pub fn parse_response_head(buf: &[u8], limits: Limits) -> Result<Option<(ResponseHeadIn, usize)>, HttpError> {
    let Some(end) = find_head_end(buf) else {
        return if buf.len() > limits.max_head_bytes { Err(HttpError::HeadTooLarge) } else { Ok(None) };
    };
    if end > limits.max_head_bytes {
        return Err(HttpError::HeadTooLarge);
    }
    let text = core::str::from_utf8(&buf[..end - 4]).map_err(|_| HttpError::Malformed("non-UTF-8 head"))?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next().ok_or(HttpError::Malformed("empty"))?;
    let minor_version = match status_line.get(..9) {
        Some("HTTP/1.1 ") => 1,
        Some("HTTP/1.0 ") => 0,
        _ => return Err(HttpError::UnsupportedVersion),
    };
    let status = status_line
        .get(9..12)
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|s| (100..600).contains(s))
        .ok_or(HttpError::Malformed("status code"))?;
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() == limits.max_headers {
            return Err(HttpError::TooManyHeaders);
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::Malformed("header line"))?;
        // An origin's line break inside a value would reach clients as a header of its own.
        if !value_ok(value) || !value_ok(name) {
            return Err(HttpError::Malformed("header"));
        }
        headers.push((name.trim().to_ascii_lowercase(), String::from(value.trim())));
    }
    one_length(&headers)?;
    Ok(Some((ResponseHeadIn { status, minor_version, headers }, end)))
}

/// Serialises a request head (`Host` must be among `headers`).
#[must_use]
pub fn request_head(method: &str, target: &str, headers: &[(&str, &str)]) -> Vec<u8> {
    let mut s = String::with_capacity(128);
    let _ = write!(s, "{method} {target} HTTP/1.1\r\n");
    write_headers(&mut s, headers);
    s.push_str("\r\n");
    s.into_bytes()
}
