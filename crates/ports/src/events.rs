//! Things an operator may want to know about (written to the event log).

use lanstash_domain::extent::Span;

/// Something an operator may want to know about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// An upstream request failed or returned something unusable.
    DownloadFailed { key: &'a str, host: &'a str, span: Option<Span>, error: &'a str },
    /// A body did not match its checksum.
    ChecksumMismatch { key: &'a str, host: &'a str },
    /// Storing failed; the response was still served.
    StoreFailed { key: &'a str, error: &'a str },
    /// The origin answered a ranged request with the whole object.
    RangeIgnored { key: &'a str, host: &'a str },
    /// A stored record carried a different key than the index promised.
    KeyMismatch { key: &'a str },
    /// An HTTPS connection was refused or could not be relayed (`host` is `-` without a name).
    TlsFailed { host: &'a str, error: &'a str },
    /// Eviction deleted a segment or object file (`last_used_ms` = its last use).
    Evicted { path: &'a str, bytes: u64, last_used_ms: u64 },
    /// Eviction could not delete something.
    EvictFailed { path: &'a str, error: &'a str },
    /// A segment was compacted: `records` objects (`kept_bytes`) copied out of
    /// it, then it was deleted, giving back `freed_bytes`.
    Compacted { path: &'a str, records: u64, kept_bytes: u64, freed_bytes: u64 },
}

/// Receives [`Event`]s.
pub trait Events: Send + Sync {
    fn emit(&self, event: &Event<'_>);
}

/// Discards all events.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoEvents;

impl Events for NoEvents {
    fn emit(&self, _event: &Event<'_>) {}
}
