//! Download coordination.
//!
//! Every download of an object is registered in one in-flight table. Planning
//! happens under the table's lock and also consults the index, so two downloads
//! never cover the same bytes, and a finished download is in the index before
//! it leaves the table. Downloads belong to no client: a client hanging up does
//! not stop them.
//!
//! - While an object's total length is unknown, one **probe** download runs; other
//!   requests wait for its response head.
//! - Missing bytes are fetched exactly as requested, in pieces of at most
//!   `piece` bytes, at most `ahead` pieces in front of the client.
//! - Whole objects below `object_file_min` go to a segment record, everything
//!   else to the object's sparse file.

pub mod counters;
mod transfer;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lanstash_domain::extent::{Span, uncovered};
use lanstash_domain::service::hosts::HostRules;
use lanstash_domain::service::{FetchMode, Profile};
use lanstash_store::index::{Lookup, StoredObject};
use lanstash_store::store::{ObjectRef, Store};

use crate::download::{DlHead, Download, HeadKind};
use counters::{FetchCounters, inc};
use lanstash_ports::clock::Clock;
use lanstash_ports::events::Events;
use lanstash_ports::upstream::Upstream;

/// Marks requests lanstash sends upstream; such a request arriving at lanstash
/// means a loop (upstream DNS pointing at the cache itself).
pub const VIA_HEADER: &str = "X-LanStash-Via";

/// Fetch settings (passed in from the composition root).
#[derive(Debug, Clone, Copy)]
pub struct FetchOptions {
    /// Largest upstream request.
    pub piece: u64,
    /// Pieces started ahead of a client's read position.
    pub ahead: usize,
    /// Bytes of a download kept in memory.
    pub window: usize,
    /// Whole objects at least this large go to object files.
    pub object_file_min: u64,
    /// Upstream read buffer.
    pub read_bytes: usize,
    /// Bodies without a length are buffered up to this size to be stored.
    pub max_unknown_length: u64,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            piece: 16 << 20,
            ahead: 4,
            window: 4 << 20,
            object_file_min: 16 << 20,
            read_bytes: 256 * 1024,
            max_unknown_length: 64 << 20,
        }
    }
}

/// What a request is about (shared with its downloads).
#[derive(Clone)]
pub struct ObjectInfo {
    pub service: String,
    pub game: String,
    pub key: String,
    /// CDN path (object file name).
    pub path: String,
    /// Upstream host and target (with query string).
    pub host: String,
    pub target: String,
    pub user_agent: Option<String>,
    pub profile: Arc<dyn Profile>,
}

impl std::fmt::Debug for ObjectInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectInfo").field("key", &self.key).field("host", &self.host).finish_non_exhaustive()
    }
}

impl ObjectInfo {
    /// The stored object, marked as used unless lanstash itself is asking
    /// (planning, prefill: [`crate::via_cache::USER_AGENT`]), so its own reads never
    /// make a game look played.
    pub(crate) fn look(&self, store: &Store) -> Lookup {
        if self.user_agent.as_deref() == Some(crate::via_cache::USER_AGENT) {
            store.lookup_quietly(&self.key)
        } else {
            store.lookup(&self.key)
        }
    }

    fn obj_ref(&self) -> ObjectRef<'_> {
        ObjectRef { service: &self.service, game: &self.game, key: &self.key }
    }
}

/// What is known about an object before a response starts.
#[derive(Debug)]
pub enum Known {
    /// In the index (possibly only partly stored).
    Stored(StoredObject),
    /// Being downloaded; the head of its first download.
    Fetching { total: Option<u64>, head: Arc<DlHead>, first: Arc<Download> },
    /// The origin answered with an error: relay that response.
    Relay { head: Arc<DlHead>, download: Arc<Download> },
    /// The requested start is past the end of the object.
    Unsatisfiable { total: u64 },
    /// Known but not servable from the cache: pass the request through.
    Unusable,
    /// The origin could not be reached.
    Failed(String),
}

/// Where the bytes at a position come from.
#[derive(Debug)]
pub enum Cover {
    /// Stored now: look it up again.
    Stored,
    Download(Arc<Download>),
    Unusable,
}

#[derive(Default)]
struct Flight {
    total: Option<u64>,
    head: Option<Arc<DlHead>>,
    probe: Option<Arc<Download>>,
    downloads: Vec<Arc<Download>>,
}

impl Flight {
    fn span_of(d: &Download, total: Option<u64>) -> Option<Span> {
        d.span().or(d.requested).or_else(|| total.map(|t| Span::new(0, t)))
    }

    fn covering(&self, pos: u64) -> Option<&Arc<Download>> {
        self.downloads
            .iter()
            .find(|d| !d.is_finished() && Self::span_of(d, self.total).is_some_and(|s| s.contains(pos)))
    }
}

/// One download in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadInfo {
    pub key: String,
    pub total: Option<u64>,
    /// The bytes it delivers (`None`: the whole object, length not known yet).
    pub span: Option<Span>,
    /// End of the bytes received so far (object offset).
    pub received: u64,
}

/// The coordinator. Shared with `Arc`.
pub struct Fetcher {
    upstream: Arc<dyn Upstream>,
    store: Arc<Store>,
    events: Arc<dyn Events>,
    opts: FetchOptions,
    pub counters: FetchCounters,
    flights: Mutex<HashMap<String, Flight>>,
    /// The hosts a redirect may lead to (none set: any).
    hosts: Option<HostRules>,
    /// Times the origin's answers (none: not timed).
    clock: Option<Arc<dyn Clock>>,
}

impl std::fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fetcher").field("opts", &self.opts).field("counters", &self.counters).finish_non_exhaustive()
    }
}

impl Fetcher {
    #[must_use]
    pub fn new(upstream: Arc<dyn Upstream>, store: Arc<Store>, events: Arc<dyn Events>, opts: FetchOptions) -> Self {
        let (counters, flights) = (FetchCounters::default(), Mutex::new(HashMap::new()));
        Self { upstream, store, events, opts, counters, flights, hosts: None, clock: None }
    }

    /// Times each download's wait for the origin's response head ([`FetchCounters::answer_wait_ms`]).
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Follows a redirect only to a host of `hosts` (a CDN's own, never one inside the network).
    #[must_use]
    pub fn with_hosts(mut self, hosts: HostRules) -> Self {
        self.hosts = Some(hosts);
        self
    }

    #[must_use]
    pub fn options(&self) -> &FetchOptions {
        &self.opts
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Flight>> {
        self.flights.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The downloads in progress, for the status page.
    #[must_use]
    pub fn downloads(&self) -> Vec<DownloadInfo> {
        let m = self.lock();
        let mut out = Vec::new();
        for (key, f) in m.iter() {
            for d in f.probe.iter().chain(f.downloads.iter()).filter(|d| !d.is_finished()) {
                if f.probe.as_ref().is_some_and(|p| Arc::ptr_eq(p, d)) && f.downloads.iter().any(|x| Arc::ptr_eq(x, d))
                {
                    continue;
                }
                let (span, received) = d.progress();
                out.push(DownloadInfo { key: key.clone(), total: f.total, span: span.or(d.requested), received });
            }
        }
        out.sort_by(|a, b| a.key.cmp(&b.key).then(a.span.cmp(&b.span)));
        out
    }

    /// Downloads in progress.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.lock().values().map(|f| f.downloads.len() + usize::from(f.probe.is_some() && f.downloads.is_empty())).sum()
    }

    /// Learns the object's total length and headers: from the index, from a
    /// download in progress, or by starting the probe and waiting for its head.
    /// The probe fetches the client's range from `want_start`, up to `want_end`
    /// (exclusive) when the client named one, and at most one piece.
    pub fn learn(self: &Arc<Self>, obj: &Arc<ObjectInfo>, want_start: u64, want_end: Option<u64>) -> Known {
        match obj.look(&self.store) {
            Lookup::Found(o) => return Known::Stored(o),
            Lookup::Unusable => return Known::Unusable,
            Lookup::Missing => {}
        }
        let probe = {
            let mut m = self.lock();
            // A download may have been indexed since the lookup above.
            match obj.look(&self.store) {
                Lookup::Found(o) => return Known::Stored(o),
                Lookup::Unusable => return Known::Unusable,
                Lookup::Missing => {}
            }
            let f = m.entry(obj.key.clone()).or_default();
            // Only downloads still running are joined: one that ended is about to leave the
            // table, and an error it relayed is not the answer to a new request.
            if let (Some(head), Some(first)) = (&f.head, f.downloads.iter().find(|d| !d.is_finished())) {
                inc(&self.counters.joined);
                return Known::Fetching { total: f.total, head: Arc::clone(head), first: Arc::clone(first) };
            }
            // Nor is a server error still being relayed: it was the answer to an earlier request,
            // and one that comes after it (a retry) asks the origin again.
            match f.probe.as_ref().filter(|p| !p.is_finished() && !p.relays_server_error()) {
                Some(p) => {
                    inc(&self.counters.joined);
                    Arc::clone(p)
                }
                None => {
                    let requested = match obj.profile.fetch_mode() {
                        FetchMode::Ranges => {
                            let end = want_start.saturating_add(self.opts.piece);
                            Some(Span::new(
                                want_start,
                                want_end.map_or(end, |e| e.clamp(want_start.saturating_add(1), end)),
                            ))
                        }
                        FetchMode::WholeObject => None,
                    };
                    let d = Arc::new(Download::new(requested, self.opts.window));
                    f.probe = Some(Arc::clone(&d));
                    self.spawn(obj, &d, true);
                    d
                }
            }
        };
        match probe.wait_head() {
            Err(e) => Known::Failed(e),
            Ok(head) => match head.kind {
                HeadKind::Content { total, .. } => Known::Fetching { total, head: Arc::clone(&head), first: probe },
                HeadKind::Unsatisfiable { total } => Known::Unsatisfiable { total },
                HeadKind::Relay { .. } => Known::Relay { head, download: probe },
            },
        }
    }

    /// The download delivering byte `pos` of an object of `total` bytes, for a
    /// client that wants everything up to `end` (exclusive). Starts the missing
    /// pieces from `pos` on, at most `ahead` pieces deep.
    pub fn cover(self: &Arc<Self>, obj: &Arc<ObjectInfo>, pos: u64, end: u64, total: u64) -> Cover {
        let mut m = self.lock();
        let stored = match obj.look(&self.store) {
            Lookup::Unusable => return Cover::Unusable,
            Lookup::Found(o) => Some(o),
            Lookup::Missing => None,
        };
        if stored.as_ref().is_some_and(|o| o.at(pos).is_some()) {
            return Cover::Stored;
        }
        let f = m.entry(obj.key.clone()).or_default();
        f.total = f.total.or(Some(total));
        if let Some(d) = f.covering(pos) {
            inc(&self.counters.joined);
            return Cover::Download(Arc::clone(d));
        }
        let mut started: Option<Arc<Download>> = None;
        match obj.profile.fetch_mode() {
            FetchMode::WholeObject => {
                let d = Arc::new(Download::new(None, self.opts.window));
                f.downloads.push(Arc::clone(&d));
                self.spawn(obj, &d, false);
                started = Some(d);
            }
            FetchMode::Ranges => {
                let horizon = end
                    .min(total)
                    .min(pos.saturating_add(self.opts.piece.saturating_mul(self.opts.ahead.max(1) as u64)));
                // A finished download's bytes are in the index by now (or failed): only running ones hold a span.
                let mut taken: Vec<Span> = f
                    .downloads
                    .iter()
                    .filter(|d| !d.is_finished())
                    .filter_map(|d| Flight::span_of(d, f.total))
                    .collect();
                if let Some(o) = &stored {
                    taken.extend(o.pieces.iter().map(|(s, _)| *s));
                }
                taken.sort();
                for gap in uncovered(Span::new(pos, horizon), &taken) {
                    let mut a = gap.start;
                    while a < gap.end {
                        let piece = Span::new(a, gap.end.min(a.saturating_add(self.opts.piece)));
                        let d = Arc::new(Download::new(Some(piece), self.opts.window));
                        f.downloads.push(Arc::clone(&d));
                        self.spawn(obj, &d, false);
                        if piece.contains(pos) {
                            started = Some(Arc::clone(&d));
                        }
                        a = piece.end;
                    }
                }
            }
        }
        match started {
            Some(d) => Cover::Download(d),
            // Nothing could be started for `pos` (e.g. it is past the end): treat as stored and let the caller re-check.
            None => Cover::Stored,
        }
    }

    fn spawn(self: &Arc<Self>, obj: &Arc<ObjectInfo>, d: &Arc<Download>, probe: bool) {
        inc(&self.counters.started);
        let (this, info, dl) = (Arc::clone(self), Arc::clone(obj), Arc::clone(d));
        let spawned = std::thread::Builder::new()
            .name("lanstash-fetch".into())
            .stack_size(256 * 1024)
            .spawn(move || this.run(&info, &dl, probe));
        if let Err(e) = spawned {
            d.finish(Err(format!("cannot start download thread: {e}")));
            self.forget(&obj.key, d, probe);
        }
    }
}
