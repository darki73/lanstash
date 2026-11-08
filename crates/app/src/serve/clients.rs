//! Bytes sent to each client per game, counted as they are sent: since the start
//! (the counters kept by the minute file them as
//! `client/<address>/<service>/<game>/<hit|miss>`, so who got what outlives a
//! restart), and over the last seconds (who is downloading what now, how fast).
//! `hit` is what came from the stash, `miss` what was sent while lanstash
//! downloaded it (a response partly stored counts as a miss, as in the
//! request counters).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Clock;
use lanstash_ports::storage::FileRead;

use super::{ResponseHead, ResponseSink, SinkError};

/// What the counters of clients' bytes are named after.
pub const PREFIX: &str = "client/";

/// A client, a service and a store directory (its game).
type Key = (String, String, String);

/// A client that got nothing of a game for this long starts a new download of it when it asks again
/// (a pause, a game deleted and fetched anew).
pub const SESSION_GAP_MS: u64 = 120_000;

/// One client's bytes of one game.
#[derive(Debug, Default)]
struct Tally {
    hit: u64,
    miss: u64,
    last_ms: u64,
    /// The download going on: its bytes, and when it started.
    session: u64,
    since_ms: u64,
    /// The last seconds: (second, from the stash, while downloading).
    recent: VecDeque<(u64, u64, u64)>,
}

/// A client downloading a game now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloading {
    pub client: String,
    pub service: String,
    pub game: String,
    /// Bytes per second over the last seconds.
    pub per_sec: u64,
    /// Of the last seconds' bytes, from the stash and while downloading.
    pub hit_bytes: u64,
    pub miss_bytes: u64,
    /// All it got of the game since the start.
    pub total_bytes: u64,
    /// What it got in this download (since it started, after a pause of [`SESSION_GAP_MS`]).
    pub session_bytes: u64,
    pub since_ms: u64,
    pub last_ms: u64,
}

/// A counter of clients' bytes, read back: client, service, game, from the stash or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCounter {
    pub client: String,
    pub service: String,
    pub game: String,
    pub hit: bool,
}

impl ClientCounter {
    /// The counter `name` (`client/<address>/<service>/<game>/<hit|miss>`), when it is one.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let rest = name.strip_prefix(PREFIX)?;
        let (rest, kind) = rest.rsplit_once('/')?;
        let (client, rest) = rest.split_once('/')?;
        let (service, game) = rest.split_once('/')?;
        let hit = match kind {
            "hit" => true,
            "miss" => false,
            _ => return None,
        };
        let ok = |s: &str| !s.is_empty();
        (ok(client) && ok(service) && ok(game)).then(|| Self {
            client: client.into(),
            service: service.into(),
            game: game.into(),
            hit,
        })
    }
}

/// Bytes sent per client and game.
pub struct ClientTraffic {
    clock: Arc<dyn Clock>,
    /// How far back "now" reaches (ms).
    window_ms: u64,
    seen: Mutex<BTreeMap<Key, Tally>>,
}

impl std::fmt::Debug for ClientTraffic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientTraffic").field("window_ms", &self.window_ms).finish_non_exhaustive()
    }
}

impl ClientTraffic {
    /// Counts by `clock`; a client is downloading a game while it got some of it in the last `window_ms`.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, window_ms: u64) -> Self {
        Self { clock, window_ms: window_ms.max(1_000), seen: Mutex::new(BTreeMap::new()) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<Key, Tally>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts `bytes` of `service`'s `game` sent to `client`, from the stash (`hit`) or not.
    pub fn add(&self, client: &str, service: &str, game: &str, hit: bool, bytes: u64) {
        if bytes == 0 {
            return;
        }
        let now = self.clock.now_ms();
        let second = now / 1_000;
        let mut seen = self.lock();
        let t = seen.entry((client.to_owned(), service.to_owned(), game.to_owned())).or_default();
        let (h, m) = if hit { (bytes, 0) } else { (0, bytes) };
        if t.session == 0 || now.saturating_sub(t.last_ms) > SESSION_GAP_MS {
            t.session = 0;
            t.since_ms = now;
        }
        t.hit += h;
        t.miss += m;
        t.session += bytes;
        t.last_ms = now;
        match t.recent.back_mut() {
            Some(last) if last.0 == second => {
                last.1 += h;
                last.2 += m;
            }
            _ => t.recent.push_back((second, h, m)),
        }
        let oldest = second.saturating_sub(self.window_ms / 1_000);
        while t.recent.front().is_some_and(|r| r.0 < oldest) {
            t.recent.pop_front();
        }
    }

    /// Every client and game sent anything since the start, as counters
    /// (`client/<address>/<service>/<game>/<hit|miss>`) and their bytes.
    #[must_use]
    pub fn counters(&self) -> Vec<(String, u64)> {
        let seen = self.lock();
        let mut out = Vec::with_capacity(seen.len() * 2);
        for ((c, s, g), t) in seen.iter() {
            for (kind, n) in [("hit", t.hit), ("miss", t.miss)] {
                if n > 0 {
                    out.push((format!("{PREFIX}{c}/{s}/{g}/{kind}"), n));
                }
            }
        }
        out
    }

    /// The clients downloading now, fastest first.
    #[must_use]
    pub fn now(&self) -> Vec<Downloading> {
        let now = self.clock.now_ms();
        let window_s = self.window_ms / 1_000;
        let oldest = (now / 1_000).saturating_sub(window_s);
        let seen = self.lock();
        let mut out: Vec<Downloading> = seen
            .iter()
            .filter(|(_, t)| now.saturating_sub(t.last_ms) <= self.window_ms)
            .map(|((c, s, g), t)| {
                let (hit, miss) =
                    t.recent.iter().filter(|r| r.0 >= oldest).fold((0, 0), |(h, m), r| (h + r.1, m + r.2));
                Downloading {
                    client: c.clone(),
                    service: s.clone(),
                    game: g.clone(),
                    per_sec: (hit + miss) / window_s.max(1),
                    hit_bytes: hit,
                    miss_bytes: miss,
                    total_bytes: t.hit + t.miss,
                    session_bytes: t.session,
                    since_ms: t.since_ms,
                    last_ms: t.last_ms,
                }
            })
            .collect();
        out.sort_by(|a, b| b.per_sec.cmp(&a.per_sec).then_with(|| a.client.cmp(&b.client)));
        out
    }

    /// Every client sent anything since the start.
    #[must_use]
    pub fn clients(&self) -> Vec<String> {
        let mut c: Vec<String> = self.lock().keys().map(|(c, _, _)| c.clone()).collect();
        c.dedup();
        c
    }
}

/// A sink that tells `count` how many body bytes went through it.
pub(super) struct Counting<'a, F: Fn(u64)> {
    pub(super) inner: &'a mut dyn ResponseSink,
    pub(super) count: F,
}

impl<F: Fn(u64)> ResponseSink for Counting<'_, F> {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        self.inner.send_head(head)
    }
    fn send_stream_head(&mut self, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
        self.inner.send_stream_head(status, headers)
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        self.inner.send_body(data)?;
        (self.count)(data.len() as u64);
        Ok(())
    }
    fn send_file(&mut self, file: &dyn FileRead, offset: u64, len: u64) -> Result<bool, SinkError> {
        let sent = self.inner.send_file(file, offset, len)?;
        if sent {
            (self.count)(len);
        }
        Ok(sent)
    }
}
