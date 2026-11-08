//! Byte ranges: `Range` requests and `Content-Range` answers.

/// A single `Range: bytes=…` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    /// `bytes=a-b` (inclusive).
    FromTo(u64, u64),
    /// `bytes=a-`
    From(u64),
    /// `bytes=-n`: the last `n` bytes.
    Suffix(u64),
}

impl ByteRange {
    /// First byte, if it doesn't depend on the object size.
    #[must_use]
    pub fn known_start(self) -> Option<u64> {
        match self {
            Self::FromTo(a, _) | Self::From(a) => Some(a),
            Self::Suffix(_) => None,
        }
    }

    /// Inclusive byte range within an object of `total` bytes; `None` if unsatisfiable.
    #[must_use]
    pub fn resolve(self, total: u64) -> Option<(u64, u64)> {
        if total == 0 {
            return None;
        }
        let (a, b) = match self {
            Self::FromTo(a, b) => (a, b.min(total - 1)),
            Self::From(a) => (a, total - 1),
            Self::Suffix(0) => return None,
            Self::Suffix(n) => (total.saturating_sub(n), total - 1),
        };
        (a <= b && a < total).then_some((a, b))
    }
}

/// Parses a single-range `Range` header. Multiple ranges and other units are
/// not supported (`None`).
#[must_use]
pub fn parse_range(v: &str) -> Option<ByteRange> {
    let spec = v.trim().strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    match (a.is_empty(), b.is_empty()) {
        (true, false) => number(b).map(ByteRange::Suffix),
        (false, true) => number(a).map(ByteRange::From),
        (false, false) => {
            let (a, b) = (number(a)?, number(b)?);
            (a <= b).then_some(ByteRange::FromTo(a, b))
        }
        (true, true) => None,
    }
}

/// Digits only (no sign: `+5` is no position HTTP knows).
fn number(s: &str) -> Option<u64> {
    let s = s.trim();
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

/// A `Content-Range: bytes start-end/total` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentRange {
    pub start: u64,
    /// Inclusive.
    pub end: u64,
    pub total: u64,
}

/// Parses `bytes start-end/total` (`None` for `*/total` and malformed values).
#[must_use]
pub fn parse_content_range(v: &str) -> Option<ContentRange> {
    let rest = v.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let r = ContentRange { start: number(start)?, end: number(end)?, total: number(total)? };
    (r.start <= r.end && r.end < r.total).then_some(r)
}

/// The total of an unsatisfied-range answer, `Content-Range: bytes */total`.
#[must_use]
pub fn parse_unsatisfied_range(v: &str) -> Option<u64> {
    number(v.trim().strip_prefix("bytes")?.trim_start().strip_prefix("*/")?)
}
