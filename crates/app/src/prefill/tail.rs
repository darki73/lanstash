//! A whole file lancache holds all but its end of: its last slice or slices
//! were never fetched (some Steam chunks are a little over the 1 MiB slice,
//! and lancache kept only the first). Its start comes out of lancache, the
//! missing end from the CDN in one range request (not through the cache, so
//! no piece of it is stored on its own); the whole file is checked, then
//! stored as one, like a download.

use std::sync::atomic::Ordering;

use lanstash_ports::upstream::{Upstream, UpstreamRequest};

use super::Shared;
use super::run::{StreamCheck, Took};
use crate::lancache_read::Next;
use crate::via_cache::USER_AGENT;

/// The most a tail may be, and never more than the start lancache holds.
pub const MAX_TAIL: u64 = 1 << 20;

impl Shared {
    /// `target` (a whole file) from lancache's start of it and the CDN's end
    /// of it, asked of `host`; [`Took::Absent`] when lancache holds no usable
    /// start of it, the end is too large, or the CDN did not give it.
    pub(super) fn take_with_tail(&self, host: &str, target: &str, check: &StreamCheck<'_>) -> Result<Took, String> {
        let (Some(l), Some(up)) = (&self.lancache, &self.tails) else { return Ok(Took::Absent) };
        let mut slices = match l.read.open_prefix(target) {
            Ok(Some(s)) => s,
            Ok(None) => return Ok(Took::Absent),
            Err(_) => return Ok(Took::Rejected),
        };
        let (held, total) = (slices.end, slices.total);
        let tail = total - held;
        if tail > MAX_TAIL || tail > held {
            return Ok(Took::Absent);
        }
        let mut body = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        loop {
            match slices.next_piece() {
                Ok(Next::End) => break,
                Ok(Next::Bytes(_, b)) => body.extend_from_slice(&b),
                Ok(Next::Gone) | Err(_) => return Ok(Took::Rejected),
            }
        }
        if body.len() as u64 != held {
            return Ok(Took::Rejected);
        }
        let Some(end) = fetch_range(up.as_ref(), host, target, held, total) else { return Ok(Took::Absent) };
        let mut c = check(0, total, &slices.meta);
        c.feed(&body);
        c.feed(&end);
        if !c.finish() {
            return Ok(Took::Rejected);
        }
        let mut p = l.put.begin(target, 0, total, total, &slices.meta)?;
        p.write(0, &body)?;
        p.write(held, &end)?;
        p.commit(true)?;
        self.bytes.fetch_add(total, Ordering::Relaxed);
        self.local.fetch_add(held, Ordering::Relaxed);
        Ok(Took::Stored(total))
    }
}

/// Bytes `start..end` of `target` from `host`, exactly; `None` when the CDN
/// gives anything else.
fn fetch_range(up: &dyn Upstream, host: &str, target: &str, start: u64, end: u64) -> Option<Vec<u8>> {
    let range = format!("bytes={start}-{}", end - 1);
    let headers = [("User-Agent", USER_AGENT), ("Range", range.as_str())];
    let req = UpstreamRequest { method: "GET", host, port: None, target, headers: &headers };
    let mut r = up.fetch(&req).ok()?;
    let want = usize::try_from(end - start).ok()?;
    // A 200 is the whole file: its end is what is wanted.
    let skip = match r.head().status {
        206 => 0,
        200 => usize::try_from(start).ok()?,
        _ => return None,
    };
    let mut got = Vec::with_capacity(want);
    let mut seen = 0usize;
    let mut buf = vec![0u8; 64 << 10];
    loop {
        let n = r.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        let from = skip.saturating_sub(seen).min(n);
        got.extend_from_slice(&buf[from..n]);
        seen += n;
        if got.len() > want {
            return None;
        }
    }
    (got.len() == want).then_some(got)
}
