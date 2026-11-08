//! Use case: answer a client request.
//!
//! Host → service → profile → cached object or pass-through. For an object the
//! total length and headers come from the index or the first download; the
//! requested range is then walked: stored parts are read from disk (header and
//! first piece together, the rest one piece ahead of the socket), missing parts
//! come from downloads ([`crate::fetch`]). A download that fails while a client
//! reads it is replaced by a new one from the client's position, up to
//! `retries` times, so the client sees one uninterrupted response.

mod body;
pub(crate) mod reply;
use reply::{empty, mark, unsatisfiable};
pub mod clients;
mod object;
pub mod refused;
pub mod served;
mod tally;

use core::net::IpAddr;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;

use lanstash_domain::extent::Span;
use lanstash_domain::http::RequestHead;
use lanstash_domain::http::range::parse_range;
use lanstash_domain::object::{object_key, path_of};
use lanstash_domain::service::hosts::HostRules;
use lanstash_domain::service::{Profile, Profiles, Route};
use lanstash_ports::storage::{FileRead, StoreError};
use lanstash_store::index::SegId;
use lanstash_store::index::StoredObject;
use lanstash_store::store::Store;

use crate::download::Download;
use crate::fetch::{Fetcher, ObjectInfo, VIA_HEADER};
use crate::passthrough::Passthrough;
use clients::ClientTraffic;
use lanstash_ports::events::Events;
use refused::RefusedHosts;
use served::ServedByGame;

/// Status line and headers of a response (written by the adapter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseHead<'a> {
    pub status: u16,
    pub reason: &'static str,
    pub headers: &'a [(&'a str, &'a str)],
    pub content_length: u64,
}

/// The connection could not be written to (or the response could not be
/// completed); the adapter closes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkError(pub String);

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "response: {}", self.0)
    }
}

impl std::error::Error for SinkError {}

/// Where a handler writes its response (implemented by the HTTP adapter, which
/// also drops the body for `HEAD` requests).
pub trait ResponseSink {
    /// # Errors
    /// Connection failure.
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError>;
    /// Starts a response without a length: the body runs until the handler
    /// returns, then the connection is closed (server-sent events).
    ///
    /// # Errors
    /// Connection failure, or streaming not supported by this sink.
    fn send_stream_head(&mut self, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
        let _ = (status, headers);
        Err(SinkError("streaming responses are not supported here".into()))
    }
    /// # Errors
    /// Connection failure.
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError>;

    /// Sends `len` bytes of `file` from `offset` as body itself, when it can do it without
    /// copying them through lanstash (the kernel moving them from the file to the socket).
    /// `false` (the default): nothing was sent; send them with [`ResponseSink::send_body`].
    ///
    /// # Errors
    /// Connection failure, possibly after some of the bytes went.
    fn send_file(&mut self, file: &dyn FileRead, offset: u64, len: u64) -> Result<bool, SinkError> {
        let _ = (file, offset, len);
        Ok(false)
    }
}

/// Driving port: the HTTP adapter calls this for every request.
pub trait RequestHandler: Send + Sync {
    /// Handles one request, writing the full response to `sink`.
    ///
    /// # Errors
    /// The response could not be completed; the connection must be closed.
    fn handle(&self, req: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError>;
}

pub const HEARTBEAT_PATH: &str = "/lanstash-heartbeat";
/// The lancache heartbeat. Prefill tools probe it and look for
/// the `X-LanCache-Processed-By` header.
pub const LANCACHE_HEARTBEAT_PATH: &str = "/lancache-heartbeat";

/// Serving settings (passed in from the composition root).
#[derive(Debug, Clone, Copy)]
pub struct ServeOptions {
    /// Read and send granularity.
    pub piece_bytes: usize,
    /// Read the next segment record ahead on sequential access.
    pub prefetch: bool,
    /// New downloads for a response whose download failed.
    pub retries: usize,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self { piece_bytes: 256 * 1024, prefetch: true, retries: 3 }
    }
}

counters! {
    /// Running totals.
    pub struct ServeCounters {
    pub requests: AtomicU64,
    /// Object responses completely served from storage.
    pub hits: AtomicU64,
    pub misses: AtomicU64,
        /// Misses whose span was partly stored already (also counted in `misses`).
        pub partial: AtomicU64,
    pub hit_bytes: AtomicU64,
    pub miss_bytes: AtomicU64,
    pub refused: AtomicU64,
    /// Responses that failed on lanstash's side (storage, a download).
    pub errors: AtomicU64,
    /// Responses the client went away from before their end.
    pub aborted: AtomicU64,
    /// Body bytes the kernel sent from the file itself (`sendfile`).
    pub sendfile_bytes: AtomicU64,
    /// Body bytes copied through lanstash (a download's, or where `sendfile` is not used).
    pub copied_bytes: AtomicU64,
    pub retries: AtomicU64,
    pub prefetch_used: AtomicU64,
    pub prefetch_wasted: AtomicU64,
    /// Requests lanstash made of itself (planning, prefill), left out of every count above.
    pub own_requests: AtomicU64,
    /// Bytes those requests read.
    pub own_bytes: AtomicU64,
}
}

type Prefetched = Result<Vec<u8>, StoreError>;

struct Prefetch {
    at: (SegId, u64),
    handle: JoinHandle<Prefetched>,
}

/// Per-connection state, owned by the adapter, opaque to it.
#[derive(Default)]
pub struct ConnState {
    /// The segment record served last on this connection.
    last: Option<(SegId, u64)>,
    prefetch: Option<Prefetch>,
    /// The client is on this machine (loopback), as the adapter saw it.
    local: bool,
    /// The client's address, as the adapter saw it.
    client: Option<IpAddr>,
    /// The request is lanstash's own (planning, prefill), not a client's.
    own: bool,
    /// The connection is HTTPS.
    secure: bool,
}

impl ConnState {
    /// State for a request lanstash makes of itself: counted apart from clients'.
    #[must_use]
    pub fn own() -> Self {
        Self { own: true, ..Self::default() }
    }

    /// State for a connection; `local`: the client is on this machine.
    #[must_use]
    pub fn new(local: bool) -> Self {
        Self { local, ..Self::default() }
    }

    /// The client is at `ip` (an IPv4 address mapped into IPv6 is taken as the IPv4 one).
    #[must_use]
    pub fn with_client(mut self, ip: IpAddr) -> Self {
        self.client = Some(ip.to_canonical());
        self
    }

    /// Whether the client is on this machine.
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.local
    }

    /// The client's address, when the adapter gave it.
    #[must_use]
    pub fn client(&self) -> Option<IpAddr> {
        self.client
    }

    /// The connection is HTTPS.
    #[must_use]
    pub fn secured(mut self) -> Self {
        self.secure = true;
        self
    }

    /// Whether the connection is HTTPS.
    #[must_use]
    pub fn is_secure(&self) -> bool {
        self.secure
    }
}

impl std::fmt::Debug for ConnState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnState").field("last", &self.last).field("prefetching", &self.prefetch.is_some()).finish()
    }
}

/// The request handler.
pub struct Cache {
    store: Arc<Store>,
    fetcher: Option<Arc<Fetcher>>,
    pass: Option<Arc<Passthrough>>,
    hosts: HostRules,
    profiles: HashMap<String, Arc<dyn Profile>>,
    events: Arc<dyn Events>,
    opts: ServeOptions,
    pub counters: ServeCounters,
    /// The hosts refused (no cache-domains list names them), and how often.
    pub refused: RefusedHosts,
    /// Bytes sent to clients per game.
    pub served: ServedByGame,
    /// Bytes sent per client and game; none: not counted.
    clients: Option<Arc<ClientTraffic>>,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").field("opts", &self.opts).field("counters", &self.counters).finish_non_exhaustive()
    }
}

/// How the body of an object response is produced.
struct Plan {
    obj: Arc<ObjectInfo>,
    span: Span,
    total: u64,
    view: Option<StoredObject>,
    /// A record already read for the response head: (seg, offset, header+first body bytes).
    first_record: Option<(SegId, u64, Vec<u8>, u32)>,
    first_download: Option<Arc<Download>>,
}

impl Cache {
    /// A handler that serves what is stored (misses are 404) until
    /// [`Cache::with_upstream`] adds fetching and pass-through.
    /// Each service `hosts` names is cached by its profile in `profiles`.
    #[must_use]
    pub fn new(
        store: Arc<Store>,
        hosts: HostRules,
        profiles: &Profiles,
        events: Arc<dyn Events>,
        opts: ServeOptions,
    ) -> Self {
        let profiles: HashMap<String, Arc<dyn Profile>> =
            hosts.services().into_iter().map(|s| (s.to_owned(), profiles.get(s))).collect();
        Self {
            store,
            fetcher: None,
            pass: None,
            hosts,
            profiles,
            events,
            opts,
            counters: ServeCounters::default(),
            refused: RefusedHosts::default(),
            served: ServedByGame::default(),
            clients: None,
        }
    }

    /// Counts the bytes sent to each client per game in `clients`.
    #[must_use]
    pub fn with_clients(mut self, clients: Arc<ClientTraffic>) -> Self {
        self.clients = Some(clients);
        self
    }

    /// The bytes sent per client and game, when they are counted.
    #[must_use]
    pub fn clients(&self) -> Option<&Arc<ClientTraffic>> {
        self.clients.as_ref()
    }

    #[must_use]
    pub fn with_upstream(mut self, fetcher: Arc<Fetcher>, pass: Arc<Passthrough>) -> Self {
        self.fetcher = Some(fetcher);
        self.pass = Some(pass);
        self
    }

    #[must_use]
    pub fn fetcher(&self) -> Option<&Arc<Fetcher>> {
        self.fetcher.as_ref()
    }

    #[must_use]
    pub fn passthrough(&self) -> Option<&Arc<Passthrough>> {
        self.pass.as_ref()
    }

    fn pass_or_404(&self, req: &RequestHead, host: &str, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        match &self.pass {
            Some(p) => p.relay(req, host, sink),
            None => empty(sink, 404, &[("X-LanStash", "NOT-CACHED")]),
        }
    }

    fn handle_inner(
        &self,
        req: &RequestHead,
        conn: &mut ConnState,
        sink: &mut dyn ResponseSink,
    ) -> Result<(), SinkError> {
        if req.method != "GET" && req.method != "HEAD" {
            return empty(sink, 405, &[("Allow", "GET, HEAD")]);
        }
        if req.target == HEARTBEAT_PATH || req.target == LANCACHE_HEARTBEAT_PATH {
            return empty(sink, 204, &[("X-LanCache-Processed-By", "lanstash")]);
        }
        if req.header(&VIA_HEADER.to_ascii_lowercase()).is_some() {
            // Our own upstream request came back to us: upstream DNS points at the cache.
            return empty(sink, 508, &[("X-LanStash", "LOOP")]);
        }
        let host = req.host().map(|h| h.trim_end_matches('.').to_ascii_lowercase());
        let Some((host, service)) =
            host.as_ref().and_then(|h| self.hosts.classify(h).map(|s| (h.clone(), s.to_owned())))
        else {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
            if let Some(h) = &host {
                self.refused.note(h);
            }
            return empty(sink, 403, &[("X-LanStash", "HOST-NOT-ALLOWED")]);
        };
        let Some(profile) = self.profiles.get(&service) else {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
            return empty(sink, 403, &[("X-LanStash", "HOST-NOT-ALLOWED")]);
        };
        let Route::Object { game, path } = profile.route(&req.target).filter_plain() else {
            return self.pass_or_404(req, &host, sink);
        };
        let range = match req.header("range") {
            None => None,
            Some(v) => match parse_range(v) {
                Some(r) => Some(r),
                None => return self.pass_or_404(req, &host, sink), // multiple ranges or other units
            },
        };
        let obj = Arc::new(ObjectInfo {
            key: object_key(profile.service(), path),
            path: path_of(path).to_owned(),
            service: profile.service().to_owned(),
            game,
            host,
            target: req.target.clone(),
            user_agent: req.header("user-agent").map(str::to_owned),
            profile: Arc::clone(profile),
        });
        self.serve_object(req, obj, range, conn, sink)
    }
}

impl RequestHandler for Cache {
    fn handle(&self, req: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        let n = if conn.own { &self.counters.own_requests } else { &self.counters.requests };
        n.fetch_add(1, Ordering::Relaxed);
        self.handle_inner(req, conn, sink)
    }
}
