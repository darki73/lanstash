//! Serving one object: its length and headers from the index or the first
//! download, the response head, and origin errors relayed as they came.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_domain::extent::Span;
use lanstash_domain::http::range::ByteRange;
use lanstash_domain::http::{RequestHead, WHOLE_BODY_HEADERS, reason_phrase};
use lanstash_domain::segment::meta::decode_meta;
use lanstash_domain::segment::record::RecordHeader;
use lanstash_ports::storage::FileRead;
use lanstash_store::index::{Lookup, Source};

use crate::download::{DlHead, Download, HeadKind};
use crate::fetch::{Known, ObjectInfo};
use crate::passthrough::bad_gateway;
use lanstash_ports::events::Event;

use super::clients::Counting;
use super::tally::Tally;
use super::{Cache, ConnState, Plan, ResponseHead, ResponseSink, SinkError, empty, unsatisfiable};

impl Cache {
    /// Relays an origin error response from a download.
    fn relay_download(
        &self,
        req: &RequestHead,
        head: &DlHead,
        d: &Download,
        sink: &mut dyn ResponseSink,
    ) -> Result<(), SinkError> {
        let HeadKind::Relay { status, content_length } = head.kind else { return bad_gateway(sink) };
        let len = match content_length {
            Some(l) => l,
            None => match d.wait_end() {
                Ok(n) => n,
                Err(_) => return bad_gateway(sink),
            },
        };
        let mut headers: Vec<(&str, &str)> = head.headers.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        super::mark(&mut headers, "UPSTREAM");
        sink.send_head(&ResponseHead {
            status,
            reason: reason_phrase(status),
            headers: &headers,
            content_length: len,
        })?;
        if req.method == "HEAD" {
            return Ok(());
        }
        let mut pos = 0;
        let mut retries = usize::MAX - 1; // no retries for relayed bodies
        while pos < len {
            let before = pos;
            pos = self.send_download(d, pos, len, sink, &mut retries)?;
            if pos == before {
                return Err(SinkError("relayed body ended early".into()));
            }
        }
        Ok(())
    }

    pub(super) fn serve_object(
        &self,
        req: &RequestHead,
        obj: Arc<ObjectInfo>,
        range: Option<ByteRange>,
        conn: &mut ConnState,
        sink: &mut dyn ResponseSink,
    ) -> Result<(), SinkError> {
        let want_start = range.and_then(ByteRange::known_start).unwrap_or(0);
        let want_end = match range {
            Some(ByteRange::FromTo(_, b)) => Some(b.saturating_add(1)),
            _ => None,
        };
        let known = match &self.fetcher {
            Some(f) => f.learn(&obj, want_start, want_end),
            None => match obj.look(&self.store) {
                Lookup::Found(o) => Known::Stored(o),
                Lookup::Unusable => Known::Unusable,
                Lookup::Missing => return empty(sink, 404, &[("X-LanStash", "MISS")]),
            },
        };
        let (total, view, head, first_download) = match known {
            Known::Unusable => return self.pass_or_404(req, &obj.host, sink),
            Known::Failed(_) => return bad_gateway(sink),
            Known::Relay { head, download } => return self.relay_download(req, &head, &download, sink),
            Known::Unsatisfiable { total } => return unsatisfiable(sink, total),
            Known::Stored(o) => (o.total, Some(o), None, None),
            Known::Fetching { total: Some(t), head, first } => (t, None, Some(head), Some(first)),
            Known::Fetching { total: None, head, first } => match first.wait_end() {
                // A body without a length: answer once its length is known.
                Ok(n) => (n, None, Some(head), Some(first)),
                Err(_) => return bad_gateway(sink),
            },
        };
        let (status, span) = match range {
            None => (200, Span::new(0, total)),
            Some(r) => match r.resolve(total) {
                Some((a, b)) => (206, Span::new(a, b + 1)),
                None => return unsatisfiable(sink, total),
            },
        };
        let hit = view.as_ref().is_some_and(|o| o.covers(span));
        // Some of it stored, the rest downloaded: still a miss, labelled PARTIAL.
        let partial = !hit && view.as_ref().is_some_and(|o| o.pieces.iter().any(|(s, _)| s.intersect(&span).is_some()));
        if !hit && self.fetcher.is_none() {
            return empty(sink, 404, &[("X-LanStash", "MISS")]);
        }

        let mut plan = Plan { obj, span, total, view, first_record: None, first_download };
        // Headers: object files keep them in memory, records in their header, downloads in their head.
        let mut meta: Vec<(String, String)> = Vec::new();
        if let Some(o) = &plan.view
            && let Some(m) = &o.meta
        {
            meta = decode_meta(m).map(|(n, v)| (n.to_owned(), v.to_owned())).collect();
        } else if let Some(h) = &head {
            meta.clone_from(&h.headers);
        } else if let Some((first_span, Source::Record { seg, reader, offset, header_len })) =
            plan.view.as_ref().and_then(|o| o.at(span.start)).cloned()
        {
            let want = if span.start == 0 { (span.len().min(self.opts.piece_bytes as u64)) as usize } else { 0 };
            let mut b = vec![0u8; header_len as usize + want];
            if reader.read_exact_at(offset, &mut b).is_err() {
                self.counters.errors.fetch_add(1, Ordering::Relaxed);
                return empty(sink, 500, &[("X-LanStash", "ERROR")]);
            }
            match RecordHeader::decode(&b[..header_len as usize])
                .ok()
                .filter(|h| h.key == plan.obj.key && h.body_len == first_span.len())
            {
                Some(h) => meta = decode_meta(&h.meta).map(|(n, v)| (n.to_owned(), v.to_owned())).collect(),
                None => {
                    self.key_mismatch(&plan.obj.key);
                    return self.pass_or_404(req, &plan.obj.host, sink);
                }
            }
            plan.first_record = Some((seg, offset, b, header_len));
        }

        let range_header;
        let mut headers: Vec<(&str, &str)> = meta
            .iter()
            .filter(|(n, _)| status == 200 || !WHOLE_BODY_HEADERS.contains(&n.as_str()))
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        headers.push(("Accept-Ranges", "bytes"));
        if status == 206 {
            range_header = format!("bytes {}-{}/{total}", span.start, span.end - 1);
            headers.push(("Content-Range", &range_header));
        }
        let state = match (hit, partial) {
            (true, _) => "HIT",
            (false, true) => "PARTIAL",
            (false, false) => "MISS",
        };
        super::mark(&mut headers, state);
        sink.send_head(&ResponseHead {
            status,
            reason: reason_phrase(status),
            headers: &headers,
            content_length: span.len(),
        })?;
        // lanstash's own reads count apart: what clients got is clients' alone.
        let own = conn.own;
        let (n, b) = if own {
            (None, &self.counters.own_bytes)
        } else if hit {
            (Some(&self.counters.hits), &self.counters.hit_bytes)
        } else {
            (Some(&self.counters.misses), &self.counters.miss_bytes)
        };
        if let Some(n) = n {
            n.fetch_add(1, Ordering::Relaxed);
        }
        if partial && !own {
            self.counters.partial.fetch_add(1, Ordering::Relaxed);
        }
        if req.method == "HEAD" {
            return Ok(());
        }
        let obj = Arc::clone(&plan.obj);
        if own {
            let r = self.send_body(&mut plan, conn, sink);
            b.fetch_add(span.len(), Ordering::Relaxed);
            return r;
        }
        // How the body went: by sendfile or copied, and whether the client stayed to its end.
        let mut tally = Tally::new(sink, &self.counters);
        let r = match (&self.clients, conn.client()) {
            (Some(clients), Some(ip)) => {
                let client = ip.to_string();
                let count = |n| clients.add(&client, &obj.service, &obj.game, hit, n);
                self.send_body(&mut plan, conn, &mut Counting { inner: &mut tally, count })
            }
            _ => self.send_body(&mut plan, conn, &mut tally),
        };
        b.fetch_add(span.len(), Ordering::Relaxed);
        self.served.add(&obj.service, &obj.game, span.len());
        if r.is_err() {
            let which = if tally.gone { &self.counters.aborted } else { &self.counters.errors };
            which.fetch_add(1, Ordering::Relaxed);
        }
        r
    }

    pub(super) fn key_mismatch(&self, key: &str) {
        self.store.index().mark_unusable(key);
        self.events.emit(&Event::KeyMismatch { key });
    }
}
