//! A request fetched from the CDN straight into the stash, checked as it
//! streams and committed only once it passes ([`super::Work::check_cdn`]): a
//! file whose bytes are not what its name says is never stored. The bytes are
//! written as they arrive (a range of any size is not held in memory) and are
//! invisible until committed; one that fails its check, or comes short, is
//! dropped and asked once more.

use std::sync::atomic::Ordering;

use lanstash_domain::http::STORED_RESPONSE_HEADERS;
use lanstash_domain::http::range::parse_content_range;
use lanstash_domain::segment::meta::encode_meta;
use lanstash_ports::upstream::{Upstream, UpstreamRequest};

use super::{Job, PieceCheck, Shared, Work};
use crate::stash_put::StashPut;
use crate::via_cache::USER_AGENT;

/// What one checked fetch gave.
pub(super) enum Fetched {
    /// Stored (its bytes counted as they came).
    Stored,
    /// The CDN has no such file (404, 410).
    Missing,
    /// Its check failed: nothing stored.
    Rejected,
    /// It could not be fetched whole: nothing stored.
    Failed(String),
    /// Its check cannot judge it: left to the cache.
    Unchecked,
}

impl Shared {
    /// Job number `i` from `host`, checked; asked once more when its check fails or it comes short.
    pub(super) fn fetch_checked(&self, host: &str, work: &Work, i: usize, job: &Job) -> Result<Fetched, String> {
        let Some((up, put)) = &self.checked else { return Ok(Fetched::Unchecked) };
        let mut last = Fetched::Unchecked;
        for _ in 0..2 {
            last = self.fetch_once(up.as_ref(), put, host, work, i, job)?;
            match last {
                Fetched::Rejected | Fetched::Failed(_) => {}
                done => return Ok(done),
            }
        }
        Ok(last)
    }

    fn fetch_once(
        &self,
        up: &dyn Upstream,
        put: &StashPut,
        host: &str,
        work: &Work,
        i: usize,
        job: &Job,
    ) -> Result<Fetched, String> {
        let asked = job.range.map(|(a, b)| format!("bytes={a}-{}", b.saturating_sub(1)));
        let mut headers = vec![("User-Agent", USER_AGENT)];
        if let Some(r) = &asked {
            headers.push(("Range", r.as_str()));
        }
        let req = UpstreamRequest { method: "GET", host, port: None, target: &job.target, headers: &headers };
        let mut resp = match up.fetch(&req) {
            Ok(r) => r,
            Err(e) => return Ok(Fetched::Failed(e.message)),
        };
        let head = resp.head();
        // Where the bytes sit in the file, and its size.
        let (start, end, total) = match (head.status, job.range) {
            (404 | 410, _) => return Ok(Fetched::Missing),
            (200, None) => match head.content_length() {
                Some(n) if n > 0 => (0, n, n),
                _ => return Ok(Fetched::Failed("no length".into())),
            },
            (206, Some((a, b))) => match head.header("content-range").and_then(parse_content_range) {
                Some(c) if c.start == a && c.end + 1 == b => (a, b, c.total),
                _ => return Ok(Fetched::Failed("another range came back".into())),
            },
            (s, _) => return Ok(Fetched::Failed(format!("status {s}"))),
        };
        let mut check: Box<dyn PieceCheck> = (work.check)(i, start, end, &[]);
        if !check.checks() {
            return Ok(Fetched::Unchecked);
        }
        let kept: Vec<(&str, &str)> = head
            .headers
            .iter()
            .filter(|(n, _)| STORED_RESPONSE_HEADERS.contains(&n.as_str()))
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .collect();
        let mut pending = put.begin(&job.target, start, end, total, &encode_meta(&kept))?;
        let (mut at, mut buf) = (start, vec![0u8; 64 << 10]);
        // What did not end stored no longer counts: the next try brings it again.
        let take_back = |at: u64| self.bytes.fetch_sub(at - start, Ordering::Relaxed);
        let short = loop {
            if self.stop.load(Ordering::SeqCst) {
                take_back(at);
                return Err("stopped".into());
            }
            let n = match resp.read(&mut buf) {
                Ok(0) => break at != end,
                Ok(n) => n,
                // Broken off: what came is not all of it.
                Err(_) => break true,
            };
            if at + n as u64 > end {
                break true;
            }
            check.feed(&buf[..n]);
            if let Err(e) = pending.write(at, &buf[..n]) {
                take_back(at);
                return Err(e);
            }
            at += n as u64;
            self.bytes.fetch_add(n as u64, Ordering::Relaxed);
        };
        // Dropped uncommitted, nothing of it is stored.
        if short {
            take_back(at);
            return Ok(Fetched::Failed(format!("{} of {} bytes came", at - start, end - start)));
        }
        if !check.finish() {
            take_back(at);
            return Ok(Fetched::Rejected);
        }
        if let Err(e) = pending.commit(true) {
            take_back(at);
            return Err(e);
        }
        Ok(Fetched::Stored)
    }
}
