//! GETs through the cache's own request path ([`RequestHandler`]), as a
//! client would send them, so what comes back is stored exactly as a
//! client's download would be, but counted as lanstash's own requests,
//! not a client's. Used by Battle.net planning and prefill.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lanstash_domain::http::RequestHead;

use crate::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};

/// The user agent these requests carry.
pub const USER_AGENT: &str = "lanstash-prefill";

/// One request.
#[derive(Debug, Clone, Copy)]
pub struct Get<'a> {
    pub host: &'a str,
    pub target: &'a str,
    /// Bytes `start..end`, or the whole file.
    pub range: Option<(u64, u64)>,
    /// Keep the body, up to this many bytes; `None` drops it.
    pub keep: Option<usize>,
}

/// Counts and stops shared with the caller.
#[derive(Debug, Clone, Copy, Default)]
pub struct Watch<'a> {
    /// Bytes received are added here.
    pub bytes: Option<&'a AtomicU64>,
    /// And here: this request's own bytes, to take back out of `bytes` when it fails.
    pub mine: Option<&'a AtomicU64>,
    /// When set, the body is broken off.
    pub stop: Option<&'a AtomicBool>,
}

struct Drain<'a> {
    status: u16,
    keep: Option<(Vec<u8>, usize)>,
    watch: Watch<'a>,
}

impl ResponseSink for Drain<'_> {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        self.status = head.status;
        Ok(())
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        if self.watch.stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
            return Err(SinkError("stopped".into()));
        }
        for b in [self.watch.bytes, self.watch.mine].into_iter().flatten() {
            b.fetch_add(data.len() as u64, Ordering::Relaxed);
        }
        if let Some((k, max)) = &mut self.keep {
            if k.len() + data.len() > *max {
                return Err(SinkError(format!("over {max} bytes")));
            }
            k.extend_from_slice(data);
        }
        Ok(())
    }
}

/// Sends `get`: `Ok(None)` when the CDN has no such file (404, 410), the
/// body (empty unless kept) otherwise.
///
/// # Errors
/// The request failed, or the answer was neither the file nor a 404.
pub fn get(cache: &dyn RequestHandler, get: Get<'_>, watch: Watch<'_>) -> Result<Option<Vec<u8>>, String> {
    let mut headers = vec![("host".into(), get.host.into()), ("user-agent".into(), USER_AGENT.into())];
    if let Some((a, b)) = get.range {
        headers.push(("range".into(), format!("bytes={a}-{}", b.saturating_sub(1))));
    }
    let req = RequestHead { method: "GET".into(), target: get.target.into(), minor_version: 1, headers };
    let mut sink = Drain { status: 0, keep: get.keep.map(|m| (Vec::new(), m)), watch };
    cache.handle(&req, &mut ConnState::own(), &mut sink).map_err(|e| format!("{}: {}", get.target, e.0))?;
    match sink.status {
        200 | 206 => Ok(Some(sink.keep.map(|(k, _)| k).unwrap_or_default())),
        404 | 410 => Ok(None),
        s => Err(format!("{}: status {s}", get.target)),
    }
}
