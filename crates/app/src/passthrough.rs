//! Requests that are not cached (manifests, version files, unusable objects)
//! are relayed to the origin as they are.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_domain::http::{RequestHead, reason_phrase};

use crate::fetch::VIA_HEADER;
use crate::serve::{ResponseHead, ResponseSink, SinkError};
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest};

counters! {
    /// Running totals.
    pub struct PassCounters {
    pub requests: AtomicU64,
    pub bytes: AtomicU64,
    /// Requests the origin failed: no answer, or a body cut short.
    pub errors: AtomicU64,
    /// Of those, the origin not connecting or answering in time.
    pub timeouts: AtomicU64,
    /// Bodies without a length past what is buffered (answered 502).
    pub too_large: AtomicU64,
    /// Responses the client went away from before their end.
    pub aborted: AtomicU64,
}
}

/// Relays requests to the origin.
pub struct Passthrough {
    upstream: Arc<dyn Upstream>,
    /// Bodies without a length are buffered up to this size.
    max_buffered: u64,
    read_bytes: usize,
    pub counters: PassCounters,
}

impl std::fmt::Debug for Passthrough {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Passthrough").field("counters", &self.counters).finish_non_exhaustive()
    }
}

/// 502 for a failed upstream request.
pub(crate) fn bad_gateway(sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
    sink.send_head(&ResponseHead {
        status: 502,
        reason: "Bad Gateway",
        headers: &[("X-LanStash", "UPSTREAM-ERROR")],
        content_length: 0,
    })
}

impl Passthrough {
    #[must_use]
    pub fn new(upstream: Arc<dyn Upstream>, max_buffered: u64, read_bytes: usize) -> Self {
        Self { upstream, max_buffered, read_bytes: read_bytes.max(4096), counters: PassCounters::default() }
    }

    /// Counts a failure of the origin.
    fn failed(&self, e: &UpstreamError) {
        self.counters.errors.fetch_add(1, Ordering::Relaxed);
        if e.timed_out {
            self.counters.timeouts.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A write to the client: counted when it failed (the client went away).
    fn client(&self, r: Result<(), SinkError>) -> Result<(), SinkError> {
        if r.is_err() {
            self.counters.aborted.fetch_add(1, Ordering::Relaxed);
        }
        r
    }

    /// Relays `req` to `host`.
    ///
    /// # Errors
    /// The client connection failed, or the upstream failed mid-body.
    pub fn relay(&self, req: &RequestHead, host: &str, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        self.counters.requests.fetch_add(1, Ordering::Relaxed);
        let mut headers = vec![(VIA_HEADER, "1")];
        for name in ["user-agent", "range", "accept", "if-modified-since", "if-none-match"] {
            if let Some(v) = req.header(name) {
                headers.push((name, v));
            }
        }
        let up = UpstreamRequest { method: &req.method, host, port: None, target: &req.target, headers: &headers };
        let mut resp = match self.upstream.fetch(&up) {
            Ok(r) => r,
            Err(e) => {
                self.failed(&e);
                return bad_gateway(sink);
            }
        };
        let head = resp.head().clone();
        let mut relayed = head.relayed();
        crate::serve::reply::mark(&mut relayed, "PASS");
        let no_body = req.method == "HEAD" || head.bodyless();
        let send_head = |sink: &mut dyn ResponseSink, len: u64| {
            sink.send_head(&ResponseHead {
                status: head.status,
                reason: reason_phrase(head.status),
                headers: &relayed,
                content_length: len,
            })
        };
        let mut buf = vec![0u8; self.read_bytes];
        match head.content_length() {
            _ if no_body => send_head(sink, head.content_length().unwrap_or(0)),
            Some(len) => {
                self.client(send_head(sink, len))?;
                let mut sent = 0u64;
                while sent < len {
                    let n = resp.read(&mut buf).map_err(|e| {
                        self.failed(&e);
                        SinkError(e.to_string())
                    })?;
                    if n == 0 {
                        self.counters.errors.fetch_add(1, Ordering::Relaxed);
                        return Err(SinkError(format!("upstream body ended at {sent} of {len} bytes")));
                    }
                    let n = n.min(usize::try_from(len - sent).unwrap_or(usize::MAX));
                    self.client(sink.send_body(&buf[..n]))?;
                    sent += n as u64;
                    self.counters.bytes.fetch_add(n as u64, Ordering::Relaxed);
                }
                Ok(())
            }
            None => {
                // Unknown length: buffer (bounded), then answer with a length.
                let mut body = Vec::new();
                loop {
                    let n = match resp.read(&mut buf) {
                        Ok(n) => n,
                        Err(e) => {
                            self.failed(&e);
                            return bad_gateway(sink);
                        }
                    };
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&buf[..n]);
                    if body.len() as u64 > self.max_buffered {
                        self.counters.too_large.fetch_add(1, Ordering::Relaxed);
                        return bad_gateway(sink);
                    }
                }
                self.client(send_head(sink, body.len() as u64))?;
                self.counters.bytes.fetch_add(body.len() as u64, Ordering::Relaxed);
                self.client(sink.send_body(&body))
            }
        }
    }
}
