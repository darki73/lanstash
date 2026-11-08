//! In-memory doubles for tests of this and other crates (feature
//! `test-support`): a scripted origin ([`FakeUpstream`]) and a response sink
//! that keeps what it is sent ([`Capture`]), and what is kept beyond a restart
//! in memory ([`MemoryLogs`]). Storage doubles live in `lanstash_store::test_support`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::http::range::parse_range;
use lanstash_ports::history::{Event, EventQuery, HistoryLog, Seen, SeenAll};
use lanstash_ports::prefill::{PrefillLog, PrefillStatus};
use lanstash_ports::stats::{Bucket, StatsLog};
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};

use crate::serve::{ResponseHead, ResponseSink, SinkError};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A canned origin response.
#[derive(Debug, Clone)]
pub struct FakeResponse {
    pub status: u16,
    /// Extra headers; `content-length` is added unless `chunked`.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Omit `content-length` (like a chunked response).
    pub chunked: bool,
    /// Serve only this many body bytes, then report a connection error.
    pub fail_after: Option<usize>,
    /// With `fail_after`: fail only this many responses, then serve normally.
    pub fail_times: Option<Arc<AtomicUsize>>,
    /// Honour `Range` (answer 206 with Content-Range).
    pub ranges: bool,
}

impl FakeResponse {
    /// 200 with `body` and the `extra` headers.
    #[must_use]
    pub fn ok(body: &[u8], extra: &[(&str, &str)]) -> Self {
        Self {
            status: 200,
            headers: extra.iter().map(|(n, v)| (n.to_ascii_lowercase(), (*v).to_owned())).collect(),
            body: body.to_vec(),
            chunked: false,
            fail_after: None,
            fail_times: None,
            ranges: false,
        }
    }

    /// Like [`FakeResponse::ok`], honouring `Range` requests.
    #[must_use]
    pub fn ranged(body: &[u8], extra: &[(&str, &str)]) -> Self {
        Self { ranges: true, ..Self::ok(body, extra) }
    }
}

/// One request the fake upstream received.
#[derive(Debug, Clone)]
pub struct SeenRequest {
    pub method: String,
    pub host: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
}

/// In-memory [`Upstream`]: routes by `host + target`, can hold responses
/// back until released (to test coalescing).
#[derive(Debug, Default)]
pub struct FakeUpstream {
    routes: Mutex<HashMap<String, FakeResponse>>,
    pub seen: Mutex<Vec<SeenRequest>>,
    held: Mutex<bool>,
    released: Condvar,
}

impl FakeUpstream {
    /// Answer requests for `host` + `target` with `r`.
    pub fn route(&self, host: &str, target: &str, r: FakeResponse) {
        lock(&self.routes).insert(format!("{host}{target}"), r);
    }

    /// The body routed for `host + target`.
    #[must_use]
    pub fn body(&self, host: &str, target: &str) -> Option<Vec<u8>> {
        lock(&self.routes).get(&format!("{host}{target}")).map(|r| r.body.clone())
    }

    /// Requests received so far.
    #[must_use]
    pub fn calls(&self) -> usize {
        lock(&self.seen).len()
    }

    /// Responses wait in `fetch` until [`FakeUpstream::release`].
    pub fn hold(&self) {
        *lock(&self.held) = true;
    }

    /// Let held responses go.
    pub fn release(&self) {
        *lock(&self.held) = false;
        self.released.notify_all();
    }
}

struct FakeBody {
    head: ResponseHeadIn,
    body: Vec<u8>,
    pos: usize,
    fail_after: Option<usize>,
}

impl UpstreamResponse for FakeBody {
    fn head(&self) -> &ResponseHeadIn {
        &self.head
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, UpstreamError> {
        let limit = self.fail_after.unwrap_or(self.body.len());
        if self.pos >= limit && limit < self.body.len() {
            return Err(UpstreamError::new("connection reset"));
        }
        // Small reads exercise the streaming paths.
        let n = buf.len().min(7).min(limit - self.pos);
        buf[..n].copy_from_slice(&self.body[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl Upstream for FakeUpstream {
    fn fetch(&self, req: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError> {
        lock(&self.seen).push(SeenRequest {
            method: req.method.to_owned(),
            host: req.host.to_owned(),
            target: req.target.to_owned(),
            headers: req.headers.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect(),
        });
        let mut held = lock(&self.held);
        while *held {
            held = self.released.wait(held).unwrap_or_else(PoisonError::into_inner);
        }
        drop(held);
        let r = lock(&self.routes)
            .get(&format!("{}{}", req.host, req.target))
            .cloned()
            .ok_or_else(|| UpstreamError::new("connection refused"))?;
        let mut headers = r.headers.clone();
        let mut status = r.status;
        let mut body = r.body.clone();
        let total = r.body.len() as u64;
        let range = req.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("range")).map(|(_, v)| *v);
        let resolved = range.and_then(parse_range).and_then(|x| x.resolve(total));
        if r.ranges
            && let Some((a, b)) = resolved
            && let (Ok(a), Ok(b)) = (usize::try_from(a), usize::try_from(b))
        {
            status = 206;
            headers.push(("content-range".into(), format!("bytes {a}-{b}/{total}")));
            body = r.body[a..=b].to_vec();
        } else if r.ranges && range.is_some() {
            status = 416;
            headers.push(("content-range".into(), format!("bytes */{total}")));
            body = Vec::new();
        }
        if !r.chunked {
            headers.push(("content-length".into(), body.len().to_string()));
        }
        let head = ResponseHeadIn { status, minor_version: 1, headers };
        let fail_after = match (&r.fail_after, &r.fail_times) {
            (Some(n), Some(left)) => {
                left.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |l| l.checked_sub(1)).is_ok().then_some(*n)
            }
            (n, _) => *n,
        };
        let body = if req.method == "HEAD" { Vec::new() } else { body };
        Ok(Box::new(FakeBody { head, body, pos: 0, fail_after }))
    }
}

/// Captures one response.
#[derive(Debug, Default)]
pub struct Capture {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub content_length: u64,
    pub body: Vec<u8>,
}

impl Capture {
    /// The first header called `name` (any case).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

impl ResponseSink for Capture {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        self.status = head.status;
        self.content_length = head.content_length;
        self.headers = head.headers.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect();
        Ok(())
    }

    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        self.body.extend_from_slice(data);
        Ok(())
    }
}

/// What is kept beyond a restart, in memory: the build history, counters by the minute, the
/// prefill jobs that ended and the names given to clients (the SQLite adapter's contract,
/// without a database).
#[derive(Debug, Default)]
pub struct MemoryLogs {
    events: Mutex<Vec<Event>>,
    seen: Mutex<SeenAll>,
    minutes: Mutex<std::collections::BTreeMap<(u64, String), u64>>,
    runs: Mutex<Vec<(String, PrefillStatus)>>,
    labels: Mutex<std::collections::BTreeMap<String, String>>,
}

impl lanstash_ports::clients::ClientLabels for MemoryLogs {
    fn labels(&self) -> Result<Vec<(String, String)>, String> {
        Ok(lock(&self.labels).iter().map(|(a, l)| (a.clone(), l.clone())).collect())
    }
    fn set_label(&self, address: &str, label: Option<&str>) -> Result<(), String> {
        let mut m = lock(&self.labels);
        match label {
            Some(l) => m.insert(address.to_owned(), l.to_owned()),
            None => m.remove(address),
        };
        Ok(())
    }
}

impl HistoryLog for MemoryLogs {
    fn seen(&self) -> Result<SeenAll, String> {
        Ok(lock(&self.seen).clone())
    }
    fn note(&self, platform: &str, events: &[Event], seen: &[Seen]) -> Result<(), String> {
        let mut all = lock(&self.seen);
        if !all.platforms.iter().any(|p| p == platform) {
            all.platforms.push(platform.to_owned());
        }
        for s in seen {
            all.games.retain(|g| !(g.platform == s.platform && g.game == s.game));
            all.games.push(s.clone());
        }
        lock(&self.events).extend_from_slice(events);
        Ok(())
    }
    fn events(&self, q: &EventQuery<'_>) -> Result<Vec<Event>, String> {
        let events = lock(&self.events);
        let wanted = |e: &&Event| {
            q.platform.is_none_or(|p| e.platform == p) && (q.games.is_empty() || q.games.contains(&e.game))
        };
        Ok(events.iter().rev().filter(wanted).take(q.limit).cloned().collect())
    }
}

impl StatsLog for MemoryLogs {
    fn add(&self, minute_ms: u64, deltas: &[(String, u64)]) -> Result<(), String> {
        let mut m = lock(&self.minutes);
        for (name, d) in deltas.iter().filter(|(_, d)| *d > 0) {
            *m.entry((minute_ms, name.clone())).or_default() += d;
        }
        Ok(())
    }
    fn series(&self, names: &[String], from_ms: u64, to_ms: u64, step_ms: u64) -> Result<Vec<Bucket>, String> {
        let step = step_ms.max(60_000);
        let mut out: Vec<Bucket> = Vec::new();
        for ((minute, name), d) in lock(&self.minutes).range((from_ms, String::new())..(to_ms, String::new())) {
            let Some(i) = names.iter().position(|n| n == name) else { continue };
            let start = from_ms + (minute - from_ms) / step * step;
            if out.last().is_none_or(|b| b.0 != start) {
                out.push((start, vec![0; names.len()]));
            }
            if let Some(b) = out.last_mut() {
                b.1[i] += d;
            }
        }
        Ok(out)
    }
    fn top(&self, prefix: &str, from_ms: u64, to_ms: u64, limit: usize) -> Result<Vec<(String, u64)>, String> {
        let mut sums: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        for ((minute, name), d) in lock(&self.minutes).iter() {
            if *minute >= from_ms && *minute < to_ms && name.starts_with(prefix) {
                *sums.entry(name.clone()).or_default() += d;
            }
        }
        let mut top: Vec<(String, u64)> = sums.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top.truncate(limit);
        Ok(top)
    }
    fn forget_before(&self, before_ms: u64) -> Result<u64, String> {
        let mut m = lock(&self.minutes);
        let n = m.len();
        m.retain(|(minute, _), _| *minute >= before_ms);
        Ok((n - m.len()) as u64)
    }
}

impl PrefillLog for MemoryLogs {
    fn record(&self, platform: &str, run: &PrefillStatus) -> Result<(), String> {
        let mut runs = lock(&self.runs);
        runs.retain(|(p, r)| !(p == platform && r.game == run.game && r.started_ms == run.started_ms));
        runs.push((platform.to_owned(), run.clone()));
        Ok(())
    }
    fn runs(&self, platform: Option<&str>, limit: usize) -> Result<Vec<(String, PrefillStatus)>, String> {
        let mut runs: Vec<_> =
            lock(&self.runs).iter().filter(|(p, _)| platform.is_none_or(|x| x == p)).cloned().collect();
        runs.sort_by_key(|(_, r)| std::cmp::Reverse(r.started_ms));
        runs.truncate(limit);
        Ok(runs)
    }
}
