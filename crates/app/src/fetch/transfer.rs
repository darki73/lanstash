//! One download on its own thread: the upstream request (following
//! redirects), the response checked against what is known, and the body
//! relayed to waiting clients and written to its record or object file.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_domain::checksum::crc::{Crc32, Crc32c};
use lanstash_domain::extent::Span;
use lanstash_domain::http::range::{parse_content_range, parse_unsatisfied_range};
use lanstash_domain::http::{ResponseHeadIn, STORED_RESPONSE_HEADERS};
use lanstash_domain::segment::meta::encode_meta;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_domain::service::BodyCheck;
use lanstash_store::game::ObjectFile;
use lanstash_store::store::RecordWriter;

use crate::download::{DlHead, Download, HeadKind};
use lanstash_ports::events::Event;
use lanstash_ports::upstream::{UpstreamRequest, UpstreamResponse};

use super::counters::inc;
use super::{Fetcher, ObjectInfo, VIA_HEADER};

const MAX_REDIRECTS: usize = 5;

/// Where a download writes.
enum Target {
    Record(RecordWriter),
    File(Arc<ObjectFile>),
}

/// Ends a download whose thread unwinds before it finished, so its waiters wake and the
/// object is free to be fetched again.
struct Unfinished<'a> {
    fetcher: &'a Fetcher,
    key: &'a str,
    d: &'a Arc<Download>,
    probe: bool,
    armed: bool,
}

impl Drop for Unfinished<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.d.finish(Err("download thread panicked".into()));
            self.fetcher.forget(self.key, self.d, self.probe);
        }
    }
}

fn write_target(t: &Target, pos: u64, data: &[u8]) -> Result<(), lanstash_ports::storage::StoreError> {
    match t {
        Target::Record(w) => w.write(pos, data),
        Target::File(f) => f.writer()?.write_all_at(pos, data),
    }
}

impl Fetcher {
    /// Download thread body.
    pub(super) fn run(&self, obj: &ObjectInfo, d: &Arc<Download>, probe: bool) {
        let mut guard = Unfinished { fetcher: self, key: &obj.key, d, probe, armed: true };
        let result = self.download(obj, d, probe);
        guard.armed = false;
        match &result {
            Ok(()) => inc(&self.counters.completed),
            Err(e) => {
                inc(&self.counters.failed);
                self.events.emit(&Event::DownloadFailed {
                    key: &obj.key,
                    host: &obj.host,
                    span: d.requested,
                    error: e,
                });
            }
        }
        d.finish(result);
        self.forget(&obj.key, d, probe);
    }

    pub(super) fn forget(&self, key: &str, d: &Arc<Download>, probe: bool) {
        let mut m = self.lock();
        if let Some(f) = m.get_mut(key) {
            f.downloads.retain(|x| !Arc::ptr_eq(x, d));
            if probe && f.probe.as_ref().is_some_and(|p| Arc::ptr_eq(p, d)) {
                f.probe = None;
            }
            if f.downloads.is_empty() && f.probe.is_none() {
                m.remove(key);
            }
        }
    }

    /// GET with redirects followed (they are never cached), only to hosts lanstash caches for.
    fn fetch_following(&self, obj: &ObjectInfo, headers: &[(&str, &str)]) -> Result<Box<dyn UpstreamResponse>, String> {
        let (mut host, mut port, mut target) = (obj.host.clone(), None, obj.target.clone());
        for _ in 0..=MAX_REDIRECTS {
            let req = UpstreamRequest { method: "GET", host: &host, port, target: &target, headers };
            let asked = self.clock.as_ref().map(|c| c.now_ms());
            let resp = self.upstream.fetch(&req).map_err(|e| {
                self.timed_out(&e);
                e.to_string()
            })?;
            let h = resp.head();
            self.answered(h.status, asked);
            if !matches!(h.status, 301 | 302 | 303 | 307 | 308) {
                return Ok(resp);
            }
            let location = h.header("location").ok_or("redirect without Location")?.to_owned();
            if let Some(rest) = location.strip_prefix("http://") {
                let (at, t) = rest.split_once('/').map_or((rest, "/".to_owned()), |(h, t)| (h, format!("/{t}")));
                let (name, p) = at.rsplit_once(':').map_or((at, None), |(n, p)| (n, p.parse::<u16>().ok()));
                if self.hosts.as_ref().is_some_and(|r| r.classify(name).is_none()) {
                    return Err(format!("redirect to {name}, a host not cached for"));
                }
                (host, port, target) = (name.to_owned(), p, t);
            } else if location.starts_with('/') {
                target = location;
            } else {
                return Err(format!("unsupported redirect to {location}"));
            }
        }
        Err("too many redirects".into())
    }

    /// Fetches, stores and publishes one download. Errors after the head was
    /// published leave already delivered bytes valid for clients.
    fn download(&self, obj: &ObjectInfo, d: &Arc<Download>, probe: bool) -> Result<(), String> {
        let range = d.requested.map(|s| format!("bytes={}-{}", s.start, s.end.saturating_sub(1)));
        let mut headers: Vec<(&str, &str)> = vec![(VIA_HEADER, "1")];
        if let Some(ua) = &obj.user_agent {
            headers.push(("User-Agent", ua));
        }
        if let Some(r) = &range {
            headers.push(("Range", r));
        }
        let mut resp = self.fetch_following(obj, &headers)?;
        let h = resp.head().clone();
        // Only the probe relays an error: a piece of an object already known carries object
        // bytes, so an error page in its place is a failed piece.
        if !probe && !matches!(h.status, 200 | 206) {
            return Err(format!("upstream answered {} for a piece", h.status));
        }

        // Where the body starts in the object, and what this download delivers.
        let (body_start, span, total) = match h.status {
            206 => {
                let cr = h
                    .header("content-range")
                    .and_then(parse_content_range)
                    .ok_or("206 without a valid Content-Range")?;
                let req = d.requested.unwrap_or(Span::new(cr.start, cr.end + 1));
                if cr.start != req.start {
                    return Err(format!("asked for bytes from {}, got {}-{}", req.start, cr.start, cr.end));
                }
                (cr.start, Span::new(cr.start, (cr.end + 1).min(req.end)), Some(cr.total))
            }
            200 => {
                let total = if h.bodyless() { Some(0) } else { h.content_length() };
                let whole = Span::new(0, total.unwrap_or(u64::MAX));
                match d.requested {
                    Some(req) if req.start > 0 || total.is_some_and(|t| req.end < t) => {
                        inc(&self.counters.range_ignored);
                        self.events.emit(&Event::RangeIgnored { key: &obj.key, host: &obj.host });
                        // The probe takes the whole object; a later piece keeps only its part.
                        if probe { (0, whole, total) } else { (0, req.intersect(&whole).unwrap_or(req), total) }
                    }
                    _ => (0, whole, total),
                }
            }
            416 => {
                let total = h.header("content-range").and_then(parse_unsatisfied_range);
                if let Some(total) = total {
                    d.set_head(
                        DlHead { kind: HeadKind::Unsatisfiable { total }, headers: vec![] },
                        Span::new(0, 0),
                        false,
                    );
                    return Ok(());
                }
                return self.relay(&mut *resp, &h, d);
            }
            _ => return self.relay(&mut *resp, &h, d),
        };

        let stored_headers: Vec<(String, String)> =
            h.headers.iter().filter(|(n, _)| STORED_RESPONSE_HEADERS.contains(&n.as_str())).cloned().collect();
        let pairs: Vec<(&str, &str)> = stored_headers.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        let meta = encode_meta(&pairs);
        let whole = total.is_some_and(|t| span == Span::new(0, t));
        let check = if whole { obj.profile.body_check(&h) } else { None };
        let head = Arc::new(DlHead { kind: HeadKind::Content { span, total }, headers: stored_headers });
        d.set_head((*head).clone(), span, check.is_some());
        {
            let mut m = self.lock();
            let f = m.entry(obj.key.clone()).or_default();
            f.total = f.total.or(total);
            f.head.get_or_insert_with(|| Arc::clone(&head));
            if probe && !f.downloads.iter().any(|x| Arc::ptr_eq(x, d)) {
                f.downloads.insert(0, Arc::clone(d));
            }
        }

        let mut target = total.and_then(|t| self.open_target(obj, d, t, whole, &meta));
        let mut unknown_body: Option<Vec<u8>> = if total.is_none() { Some(Vec::new()) } else { None };
        let mut crc_c = Crc32c::new();
        let mut crc_ieee = Crc32::new();
        let mut buf = vec![0u8; self.opts.read_bytes.max(4096)];
        let mut body_pos = body_start; // object offset of the next body byte from upstream
        let mut pos = span.start; // next delivered byte
        let outcome: Result<(), String> = loop {
            if pos >= span.end {
                break Ok(());
            }
            let n = match resp.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => n,
                Err(e) => {
                    self.timed_out(&e);
                    break Err(e.to_string());
                }
            };
            self.counters.fetched_bytes.fetch_add(n as u64, Ordering::Relaxed);
            let chunk_start = body_pos;
            body_pos += n as u64;
            // Keep only the bytes inside `span` (an origin that ignored `Range` sends more).
            let a = span.start.max(chunk_start);
            let b = span.end.min(body_pos);
            if a >= b {
                continue;
            }
            let part =
                &buf[usize::try_from(a - chunk_start).unwrap_or(0)..usize::try_from(b - chunk_start).unwrap_or(0)];
            crc_c.update(part);
            if check.is_some() {
                crc_ieee.update(part);
            }
            if let Some(t) = &target
                && let Err(e) = write_target(t, pos, part)
            {
                self.store_failed(obj, &e.to_string());
                d.clear_read_back();
                target = None;
            }
            if let Some(body) = &mut unknown_body {
                if body.len() as u64 + part.len() as u64 <= self.opts.max_unknown_length {
                    body.extend_from_slice(part);
                } else {
                    unknown_body = None;
                }
            }
            d.push(part);
            pos += part.len() as u64;
        };

        if let Err(e) = outcome {
            // Keep a large enough intact prefix of an object-file piece.
            if let Some(Target::File(file)) = &target
                && pos - span.start >= 1 << 20
            {
                self.commit_file(obj, file, Span::new(span.start, pos), crc_c.finish());
            }
            return Err(e);
        }
        if span.end != u64::MAX && pos != span.end {
            return Err(format!("upstream sent {} of {} bytes", pos - span.start, span.len()));
        }
        let mut flags = RecordFlags::default();
        if let Some(BodyCheck::Crc32(expected)) = check {
            let actual = crc_ieee.finish();
            if actual != expected {
                inc(&self.counters.crc_mismatch);
                self.events.emit(&Event::ChecksumMismatch { key: &obj.key, host: &obj.host });
                return Err(format!("x-content-crc mismatch: expected {expected}, got {actual}"));
            }
            flags.0 |= RecordFlags::UPSTREAM_VERIFIED;
        }
        match target {
            Some(Target::Record(w)) => match w.commit(flags, crc_c.finish()) {
                Ok(()) => {
                    inc(&self.counters.stored_records);
                    self.counters.stored_bytes.fetch_add(span.len(), Ordering::Relaxed);
                }
                Err(e) => self.store_failed(obj, &e.to_string()),
            },
            Some(Target::File(file)) => self.commit_file(obj, &file, span, crc_c.finish()),
            None => {
                if let Some(body) = unknown_body {
                    self.store_unknown_length(obj, &meta, &body, flags);
                }
            }
        }
        Ok(())
    }

    /// Opens where a download's bytes go; `None` when storing is not possible.
    fn open_target(&self, obj: &ObjectInfo, d: &Download, total: u64, whole: bool, meta: &[u8]) -> Option<Target> {
        let opened = if whole && total < self.opts.object_file_min {
            self.store.reserve_record(obj.obj_ref(), meta, total).map(Target::Record)
        } else {
            self.store.object_file(obj.obj_ref(), &obj.path, total, meta).map(Target::File)
        };
        match opened {
            Ok(t) => {
                match &t {
                    Target::Record(w) => {
                        let (reader, base) = w.read_back();
                        d.set_read_back(reader, base);
                    }
                    Target::File(f) => match f.reader() {
                        Ok(r) => d.set_read_back(r, 0),
                        Err(e) => self.store_failed(obj, &e.to_string()),
                    },
                }
                Some(t)
            }
            Err(e) => {
                self.store_failed(obj, &e.to_string());
                None
            }
        }
    }

    fn commit_file(&self, obj: &ObjectInfo, file: &ObjectFile, span: Span, crc: u32) {
        match self.store.commit_extent(file, span, crc) {
            Ok(()) => {
                inc(&self.counters.stored_extents);
                self.counters.stored_bytes.fetch_add(span.len(), Ordering::Relaxed);
            }
            Err(e) => self.store_failed(obj, &e.to_string()),
        }
    }

    fn store_unknown_length(&self, obj: &ObjectInfo, meta: &[u8], body: &[u8], flags: RecordFlags) {
        let len = body.len() as u64;
        if len >= self.opts.object_file_min {
            return;
        }
        let stored = self.store.reserve_record(obj.obj_ref(), meta, len).and_then(|w| {
            w.write(0, body)?;
            w.commit(flags, lanstash_domain::checksum::crc::crc32c(body))
        });
        match stored {
            Ok(()) => {
                inc(&self.counters.stored_records);
                self.counters.stored_bytes.fetch_add(len, Ordering::Relaxed);
            }
            Err(e) => self.store_failed(obj, &e.to_string()),
        }
    }

    fn store_failed(&self, obj: &ObjectInfo, error: &str) {
        inc(&self.counters.store_errors);
        self.events.emit(&Event::StoreFailed { key: &obj.key, error });
    }

    /// Relays a non-content response (404, 5xx, …) to the waiting clients.
    fn relay(&self, resp: &mut dyn UpstreamResponse, h: &ResponseHeadIn, d: &Download) -> Result<(), String> {
        let content_length = if h.bodyless() { Some(0) } else { h.content_length() };
        let headers = h.relayed().into_iter().map(|(n, v)| (n.to_owned(), v.to_owned())).collect();
        let head = DlHead { kind: HeadKind::Relay { status: h.status, content_length }, headers };
        d.set_head(head, Span::new(0, content_length.unwrap_or(u64::MAX)), false);
        let mut buf = vec![0u8; 64 * 1024];
        let mut got = 0u64;
        loop {
            if content_length.is_some_and(|l| got >= l) {
                return Ok(());
            }
            let n = resp.read(&mut buf).map_err(|e| {
                self.timed_out(&e);
                e.to_string()
            })?;
            if n == 0 {
                return match content_length {
                    Some(l) if got < l => Err(format!("upstream sent {got} of {l} bytes")),
                    _ => Ok(()),
                };
            }
            d.push(&buf[..n]);
            got += n as u64;
        }
    }
}
