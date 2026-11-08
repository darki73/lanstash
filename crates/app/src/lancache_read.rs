//! Reads bytes of a CDN file out of lancache's nginx cache, by the keys nginx
//! gave its slices (`<service><uri>bytes=<a>-<b>`, see
//! [`lanstash_domain::nginx_cache`]): no walk of the cache, one open per
//! slice. Only the slices covering the asked range are read, one at a time
//! ([`LancacheRead::open`], then [`Slices::next_piece`]), so a file of any size
//! takes one slice of memory. Read-only.
//!
//! Before the first byte, every slice the range needs must exist (a stat
//! each), so a read either has all of it or is not started. A slice that is
//! missing, or not a usable answer (a status other than 200 or 206), makes
//! [`LancacheRead::open`] come back empty: the caller fetches the piece
//! elsewhere. A slice that contradicts its key (another key, another offset,
//! another total) is an error. What comes back is not checked against any
//! hash here: that is the platform's part.
//!
//! A slice may hold more than its slice: when the origin answered the slice's
//! range request with the whole file (200), nginx kept all of it under the
//! first slice's key. Such a slice gives everything it holds, and the slices
//! it covers are neither needed nor read (lancache can hold them too).

use std::sync::Arc;

use lanstash_domain::nginx_cache::{NginxCacheFile, cache_file_path, slice_key, slice_starts};
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreErrorKind};

use crate::nginx_file::{meta, read_cache_file};

/// Bytes `start..start + bytes.len()` of a file of `total` bytes, with the
/// response headers lanstash keeps (from its first slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub start: u64,
    pub bytes: Vec<u8>,
    pub total: u64,
    pub meta: Vec<u8>,
}

/// Where lancache's slices are and how they are keyed.
#[derive(Debug, Clone)]
pub struct LancacheOptions {
    /// The cache identifier nginx used for this platform (lancache: the service name).
    pub service: String,
    /// nginx `slice` size (lancache: 1 MiB).
    pub slice_bytes: u64,
    /// [`LancacheRead::read`] (which holds a piece whole) reads no larger pieces.
    pub max_piece: u64,
}

pub struct LancacheRead {
    source: Arc<dyn NginxCacheSource>,
    opts: LancacheOptions,
}

impl std::fmt::Debug for LancacheRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LancacheRead").field("opts", &self.opts).finish_non_exhaustive()
    }
}

/// A slice file, open, and its head.
type Opened = (Arc<dyn FileRead>, NginxCacheFile);

/// What [`Slices::next_piece`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Bytes of the range, from this offset.
    Bytes(u64, Vec<u8>),
    /// A slice went missing since the range was opened.
    Gone,
    /// The range is done.
    End,
}

/// The slices of a range, read one at a time.
pub struct Slices<'a> {
    read: &'a LancacheRead,
    base: String,
    /// The range: `start..end` of a file of `total` bytes.
    pub start: u64,
    pub end: u64,
    pub total: u64,
    /// The response headers lanstash keeps (from the first slice).
    pub meta: Vec<u8>,
    /// The next slice's start, and the first slice already opened.
    at: u64,
    first: Option<Opened>,
}

impl LancacheRead {
    #[must_use]
    pub fn new(source: Arc<dyn NginxCacheSource>, opts: LancacheOptions) -> Self {
        Self { source, opts }
    }

    fn slice(&self) -> u64 {
        self.opts.slice_bytes.max(1)
    }

    /// One slice file and its head, checked against its key; `None` when it
    /// is missing or not a usable answer.
    fn open_slice(&self, key: &str, at: u64) -> Result<Option<Opened>, String> {
        let file = match self.source.open(&cache_file_path(key)) {
            Ok(f) => f,
            Err(e) if e.kind == StoreErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{key}: {e}")),
        };
        let head = read_cache_file(&*file).map_err(|e| format!("{key}: {}", e.message()))?;
        if head.key != key {
            return Err(format!("{key} is stored as {}", head.key));
        }
        if !matches!(head.status, 200 | 206) {
            return Ok(None);
        }
        let from = head.content_range().map_or(0, |r| r.start);
        if from != at {
            return Err(format!("{key}: holds bytes from {from}"));
        }
        Ok(Some((file, head)))
    }

    /// Opens bytes `range` (`start..end`) of `target` (a CDN path), or the
    /// whole file, once every slice they need is there; `None` when lancache
    /// does not hold all of it.
    ///
    /// # Errors
    /// A slice cannot be read, or contradicts its key.
    pub fn open(&self, target: &str, range: Option<(u64, u64)>) -> Result<Option<Slices<'_>>, String> {
        let slice = self.slice();
        let base = format!("{}{target}", self.opts.service);
        let start = range.map_or(0, |r| r.0);
        if range.is_some_and(|(a, b)| b <= a) {
            return Ok(None);
        }
        let first_at = start / slice * slice;
        let Some((file, head)) = self.open_slice(&slice_key(&base, first_at, slice), first_at)? else {
            return Ok(None);
        };
        let total = head.content_range().map_or(head.body_len, |r| r.total);
        let end = range.map_or(total, |r| r.1);
        if end > total {
            return Err(format!("{target}: the file has {total} bytes, {end} asked"));
        }
        // Every other slice must be there before a byte is handed over (but
        // for those the first one already holds).
        let covered = first_at + head.body_len;
        for s in slice_starts(start, end, slice).skip(1).filter(|s| *s >= covered) {
            let rel = cache_file_path(&slice_key(&base, s, slice));
            if !self.source.exists(&rel).map_err(|e| format!("{rel}: {e}"))? {
                return Ok(None);
            }
        }
        let meta = meta(&head);
        Ok(Some(Slices { read: self, base, start, end, total, meta, at: first_at, first: Some((file, head)) }))
    }

    /// The start of `target` that lancache holds when it lacks the rest: its
    /// slices from the first up to the first one missing (`end` of the
    /// [`Slices`] is where that is). `None` when it holds no first slice, or
    /// every slice (then [`Self::open`] reads it).
    ///
    /// # Errors
    /// The first slice cannot be read, or contradicts its key.
    pub fn open_prefix(&self, target: &str) -> Result<Option<Slices<'_>>, String> {
        let slice = self.slice();
        let base = format!("{}{target}", self.opts.service);
        let Some((file, head)) = self.open_slice(&slice_key(&base, 0, slice), 0)? else {
            return Ok(None);
        };
        let total = head.content_range().map_or(head.body_len, |r| r.total);
        let mut end = head.body_len.min(total);
        while end < total && end % slice == 0 {
            let rel = cache_file_path(&slice_key(&base, end, slice));
            if !self.source.exists(&rel).map_err(|e| format!("{rel}: {e}"))? {
                break;
            }
            end = (end + slice).min(total);
        }
        if end >= total || end == 0 {
            return Ok(None);
        }
        let meta = meta(&head);
        Ok(Some(Slices { read: self, base, start: 0, end, total, meta, at: 0, first: Some((file, head)) }))
    }

    /// Bytes `range` of `target`, or the whole file, held whole (at most
    /// `max_piece` bytes); `None` when lancache does not hold all of it.
    ///
    /// # Errors
    /// As [`Self::open`] and [`Slices::next_piece`].
    pub fn read(&self, target: &str, range: Option<(u64, u64)>) -> Result<Option<Piece>, String> {
        if range.is_some_and(|(a, b)| b > a && b - a > self.opts.max_piece) {
            return Ok(None);
        }
        let Some(mut s) = self.open(target, range)? else { return Ok(None) };
        if s.end - s.start > self.opts.max_piece {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        loop {
            match s.next_piece()? {
                Next::Bytes(_, part) => bytes.extend_from_slice(&part),
                Next::Gone => return Ok(None),
                Next::End => break,
            }
        }
        Ok(Some(Piece { start: s.start, bytes, total: s.total, meta: s.meta }))
    }
}

impl std::fmt::Debug for Slices<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slices")
            .field("base", &self.base)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

impl Slices<'_> {
    /// The next slice's bytes within the range ([`Next`]).
    ///
    /// # Errors
    /// A slice cannot be read, or contradicts its key or the total.
    pub fn next_piece(&mut self) -> Result<Next, String> {
        let slice = self.read.slice();
        if self.at >= self.end {
            return Ok(Next::End);
        }
        let at = self.at;
        self.at += slice;
        let opened = match self.first.take() {
            Some(f) => Some(f),
            None => self.read.open_slice(&slice_key(&self.base, at, slice), at)?,
        };
        let Some((file, head)) = opened else { return Ok(Next::Gone) };
        let of = head.content_range().map_or(head.body_len, |r| r.total);
        if of != self.total {
            return Err(format!("{}: slice at {at} says the file has {of} bytes, not {}", self.base, self.total));
        }
        let slice_end = at + head.body_len;
        if slice_end < self.end.min(self.total) && head.body_len != slice {
            return Err(format!("{}: a short slice ({} bytes) at {at}", self.base, head.body_len));
        }
        // A slice holding more than its slice (a whole-file answer) covers the next ones too.
        self.at = self.at.max(slice_end.div_ceil(slice) * slice);
        let (a, b) = (self.start.max(at), self.end.min(slice_end));
        if a >= b {
            return Ok(Next::Bytes(a, Vec::new()));
        }
        let n = usize::try_from(b - a).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; n];
        file.read_exact_at(head.body_start + (a - at), &mut buf).map_err(|e| format!("{}: {e}", self.base))?;
        Ok(Next::Bytes(a, buf))
    }
}
