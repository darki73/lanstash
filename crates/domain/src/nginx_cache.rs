//! Reading nginx proxy-cache files (as lancache's nginx writes them), for
//! importing an existing cache.
//!
//! Layout (nginx 1.24 / x86_64, cache header version 5):
//!
//! ```text
//! 0     u64  version (= 5)
//! 8..48      valid/updating/error/last_modified/date (time_t ×5)
//! 48    u32  crc32 (of the key)
//! 52    u16  valid_msec
//! 54    u16  header_start   → first byte of the stored HTTP status line
//! 56    u16  body_start     → first body byte
//! …          etag, vary, variant (struct padded to 336 bytes)
//! 336        "\nKEY: " <key> "\n"
//! header_start  "HTTP/1.1 206 Partial Content\r\n" headers "\r\n"
//! body_start    body
//! ```
//!
//! File names are `md5(key)` in `levels=2:2` directories: last two hex chars,
//! then the two before them, e.g. `7a/00/009e7e74e5f962fcbd151381e957007a`.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::checksum::md5::md5_hex;

/// Cache header version this parser understands.
pub const CACHE_VERSION: u64 = 5;
/// Size of nginx's binary cache header struct on x86_64.
pub const HEADER_STRUCT_LEN: usize = 336;
/// Bytes needed to read `header_start`/`body_start`.
pub const PREFIX_LEN: usize = 58;

/// Precedes the cache key in the file header.
pub const KEY_MARKER: &[u8] = b"\nKEY: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NginxCacheError {
    TooShort { needed: usize, got: usize },
    UnsupportedVersion(u64),
    BadLayout(&'static str),
    ContentLengthMismatch { header: u64, actual: u64 },
}

impl fmt::Display for NginxCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { needed, got } => write!(f, "nginx cache file: need {needed} bytes, got {got}"),
            Self::UnsupportedVersion(v) => write!(f, "nginx cache header version {v} not supported"),
            Self::BadLayout(what) => write!(f, "nginx cache file: {what}"),
            Self::ContentLengthMismatch { header, actual } => {
                write!(f, "nginx cache file: Content-Length {header} but body is {actual} bytes")
            }
        }
    }
}

impl core::error::Error for NginxCacheError {}

/// A parsed cache file (everything except the body itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NginxCacheFile {
    /// Full nginx cache key, e.g. `steam/depot/1/chunk/<sha>bytes=0-1048575`.
    pub key: String,
    pub status: u16,
    /// Stored response headers in order, names as stored.
    pub headers: Vec<(String, String)>,
    pub body_start: u64,
    pub body_len: u64,
}

impl NginxCacheFile {
    /// First header value with this name (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// `Content-Range: bytes start-end/total`, if present and well-formed.
    #[must_use]
    pub fn content_range(&self) -> Option<ContentRange> {
        parse_content_range(self.header("content-range")?)
    }
}

use crate::http::range::{ContentRange, parse_content_range};

/// Reads `body_start` from the first [`PREFIX_LEN`] bytes, so the caller knows
/// how much to read for [`parse`].
///
/// # Errors
/// Short buffer or unsupported version.
pub fn body_start(prefix: &[u8]) -> Result<u16, NginxCacheError> {
    if prefix.len() < PREFIX_LEN {
        return Err(NginxCacheError::TooShort { needed: PREFIX_LEN, got: prefix.len() });
    }
    let version = u64::from_le_bytes(prefix[0..8].try_into().unwrap_or([0; 8]));
    if version != CACHE_VERSION {
        return Err(NginxCacheError::UnsupportedVersion(version));
    }
    Ok(u16::from_le_bytes([prefix[56], prefix[57]]))
}

/// Parses a cache file from its first `body_start` bytes and total size.
///
/// # Errors
/// Short buffer, unsupported version, unexpected layout, or a body length that
/// disagrees with the stored `Content-Length`.
pub fn parse(head: &[u8], file_size: u64) -> Result<NginxCacheFile, NginxCacheError> {
    let body_start = usize::from(body_start(head)?);
    let header_start = usize::from(u16::from_le_bytes([head[54], head[55]]));
    if head.len() < body_start {
        return Err(NginxCacheError::TooShort { needed: body_start, got: head.len() });
    }
    if !(HEADER_STRUCT_LEN + KEY_MARKER.len() < header_start && header_start < body_start) {
        return Err(NginxCacheError::BadLayout("header_start/body_start out of order"));
    }
    if (body_start as u64) > file_size {
        return Err(NginxCacheError::BadLayout("body_start beyond end of file"));
    }
    if &head[HEADER_STRUCT_LEN..HEADER_STRUCT_LEN + KEY_MARKER.len()] != KEY_MARKER {
        return Err(NginxCacheError::BadLayout("no KEY line at offset 336"));
    }
    let key_bytes = &head[HEADER_STRUCT_LEN + KEY_MARKER.len()..header_start - 1];
    if head[header_start - 1] != b'\n' {
        return Err(NginxCacheError::BadLayout("KEY line not terminated before header_start"));
    }
    let key = core::str::from_utf8(key_bytes).map_err(|_| NginxCacheError::BadLayout("key not UTF-8"))?;

    let http = core::str::from_utf8(&head[header_start..body_start])
        .map_err(|_| NginxCacheError::BadLayout("stored headers not UTF-8"))?;
    let mut lines = http.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status = status_line
        .strip_prefix("HTTP/1.")
        .and_then(|s| s.get(2..5))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or(NginxCacheError::BadLayout("bad status line"))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (n, v) = line.split_once(':').ok_or(NginxCacheError::BadLayout("bad header line"))?;
        headers.push((String::from(n.trim()), String::from(v.trim())));
    }
    let file = NginxCacheFile {
        key: String::from(key),
        status,
        headers,
        body_start: body_start as u64,
        body_len: file_size - body_start as u64,
    };
    if let Some(cl) = file.header("content-length").and_then(|v| v.parse::<u64>().ok())
        && cl != file.body_len
    {
        return Err(NginxCacheError::ContentLengthMismatch { header: cl, actual: file.body_len });
    }
    Ok(file)
}

/// Relative path of the cache file for `key` (`levels=2:2`).
#[must_use]
pub fn cache_file_path(key: &str) -> String {
    let h = md5_hex(key.as_bytes());
    let mut p = String::with_capacity(38);
    p.push(char::from(h[30]));
    p.push(char::from(h[31]));
    p.push('/');
    p.push(char::from(h[28]));
    p.push(char::from(h[29]));
    p.push('/');
    for &c in &h {
        p.push(char::from(c));
    }
    p
}

/// Splits a sliced cache key into its base and the `bytes=start-end` suffix.
#[must_use]
pub fn split_slice_key(key: &str) -> (&str, Option<(u64, u64)>) {
    if let Some(i) = key.rfind("bytes=") {
        let (base, range) = (&key[..i], &key[i + 6..]);
        if let Some((a, b)) = range.split_once('-')
            && let (Ok(a), Ok(b)) = (a.parse(), b.parse())
        {
            return (base, Some((a, b)));
        }
    }
    (key, None)
}

/// Starts of the slices (of `slice_size` bytes) holding bytes `start..end`;
/// none when the range is empty or the slice size is 0.
pub fn slice_starts(start: u64, end: u64, slice_size: u64) -> impl Iterator<Item = u64> {
    let (first, count) = if slice_size == 0 || end <= start {
        (0, 0)
    } else {
        let first = start / slice_size;
        (first, (end - 1) / slice_size - first + 1)
    };
    (0..count).map(move |i| (first + i) * slice_size)
}

/// Cache key of the slice starting at `start` (slices of `slice_size` bytes).
#[must_use]
pub fn slice_key(base: &str, start: u64, slice_size: u64) -> String {
    alloc::format!("{base}bytes={start}-{}", start + slice_size - 1)
}
