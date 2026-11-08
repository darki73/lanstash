//! Use case: a snapshot of what the cache is doing, for the status page.
//! Plain data; rendering is up to the adapter.

use std::sync::{Arc, Mutex, PoisonError};

use lanstash_store::index::Usage;
use lanstash_store::layout::GameDir;
use lanstash_store::store::Store;

use crate::client_names::ClientName;
use crate::delete_game::DeleteStatus;
use crate::evict::Evictor;
use crate::fetch::DownloadInfo;
use crate::lifetime::Lifetime;
use crate::patrol::Patrol;
use crate::serve::Cache;
use crate::serve::clients::Downloading;
use lanstash_ports::clock::Clock;
use lanstash_ports::names::GameNames;
use lanstash_ports::prefill::PrefillStatus;

/// A group of named counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterGroup {
    pub name: &'static str,
    pub values: Vec<(&'static str, u64)>,
}

/// Stored objects and bytes of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRow {
    pub game: GameDir,
    pub usage: Usage,
    /// Display name, if a [`GameNames`] source knows it.
    pub name: Option<String>,
    /// The game this directory belongs to (a Steam depot's app id), if known.
    pub group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub now_ms: u64,
    pub started_ms: u64,
    pub counters: Vec<CounterGroup>,
    pub downloads: Vec<DownloadInfo>,
    /// Stored objects and bytes per game, as of `usage_ms`.
    pub usage: Vec<UsageRow>,
    pub usage_ms: u64,
    /// The last prefill, running or finished, and its platform (filled by whoever runs prefills).
    pub prefill: Option<(String, PrefillStatus)>,
    /// Each platform's last prefill, by platform (filled by whoever runs prefills).
    pub prefills: Vec<(String, PrefillStatus)>,
    /// The last deletion of a game, running or finished (filled by whoever deletes).
    pub deleting: Option<DeleteStatus>,
    /// When each platform's builds last changed, by platform (filled by whoever has the platforms).
    pub builds_changed: Vec<(String, u64)>,
    /// The clients downloading now and what they are called (filled by whoever names them).
    pub clients: Vec<(Downloading, ClientName)>,
}

/// Per-game usage, when it was computed, and the stash's change count then; when its names
/// were last looked up.
type UsageAt = (u64, u64, Vec<UsageRow>, u64);

/// How often the names of the per-game usage are looked up again between walks (a platform's
/// names come in after the start: Steam's once its library is read). A few hundred lookups,
/// where a walk of the index takes seconds on a large stash.
pub const NAMES_MAX_AGE_MS: u64 = 30_000;

/// The longest the per-game usage goes without a walk when the stash did not change: what
/// moves then is only the last-use times. A walk of a large stash takes seconds of CPU,
/// too much to repeat for every refresh of an open web page.
pub const USAGE_MAX_AGE_MS: u64 = 10 * 60_000;

/// Builds snapshots. The per-game usage walks the whole index, so it is
/// computed again only once the stash changed (at most once per
/// `usage_min_age_ms`), and at least once per `usage_max_age_ms` (when the
/// games were last used).
pub struct Status {
    cache: Arc<Cache>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    names: Arc<dyn GameNames>,
    started_ms: u64,
    usage_max_age_ms: u64,
    usage_min_age_ms: u64,
    usage: Mutex<Option<UsageAt>>,
    evictor: Option<Arc<Evictor>>,
    lifetime: Option<Arc<Lifetime>>,
    patrol: Option<Arc<Patrol>>,
}

impl std::fmt::Debug for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Status").field("started_ms", &self.started_ms).finish_non_exhaustive()
    }
}

impl Status {
    #[must_use]
    pub fn new(
        cache: Arc<Cache>,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        names: Arc<dyn GameNames>,
        usage_max_age_ms: u64,
    ) -> Self {
        let started_ms = clock.now_ms();
        Self {
            cache,
            store,
            clock,
            names,
            started_ms,
            usage_max_age_ms,
            usage_min_age_ms: 0,
            usage: Mutex::new(None),
            evictor: None,
            lifetime: None,
            patrol: None,
        }
    }

    /// Adds the background store check (`verify`: where its round stands, its problems) to every snapshot.
    #[must_use]
    pub fn with_patrol(mut self, patrol: Arc<Patrol>) -> Self {
        self.patrol = Some(patrol);
        self
    }

    /// Adds the lifetime totals (`lifetime`: served and fetched bytes, since when) to every snapshot.
    #[must_use]
    pub fn with_lifetime(mut self, lifetime: Arc<Lifetime>) -> Self {
        self.lifetime = Some(lifetime);
        self
    }

    /// After a change of the stash, waits `ms` since the last walk before walking again
    /// (a download changes it all the time). Without, a change is walked at once.
    #[must_use]
    pub fn with_usage_min_age(mut self, ms: u64) -> Self {
        self.usage_min_age_ms = ms;
        self
    }

    /// The cache the snapshots are of.
    #[must_use]
    pub fn cache(&self) -> &Arc<Cache> {
        &self.cache
    }

    /// Adds the eviction counters to every snapshot.
    #[must_use]
    pub fn with_evictor(mut self, evictor: Arc<Evictor>) -> Self {
        self.evictor = Some(evictor);
        self
    }

    /// The counter groups alone: what a snapshot has but the per-game usage (no walk of the index).
    #[must_use]
    pub fn counters(&self) -> Vec<CounterGroup> {
        let mut counters = vec![CounterGroup { name: "requests", values: self.cache.counters.values() }];
        if let Some(f) = self.cache.fetcher() {
            counters.push(CounterGroup { name: "downloads", values: f.counters.values() });
        }
        if let Some(p) = self.cache.passthrough() {
            counters.push(CounterGroup { name: "pass-through", values: p.counters.values() });
        }
        if let Some(e) = &self.evictor {
            counters.push(CounterGroup { name: "eviction", values: e.counters.values() });
        }
        // What is held now (gauges): segments and object files apart, and objects.
        let held = self.store.index().counts();
        counters.push(CounterGroup {
            name: "storage",
            values: vec![("segments", held.segments), ("object_files", held.object_files), ("objects", held.objects)],
        });
        if let Some(l) = &self.lifetime {
            counters.push(CounterGroup { name: "lifetime", values: l.values() });
        }
        if let Some(p) = &self.patrol {
            counters.push(CounterGroup { name: "verify", values: p.values() });
        }
        counters
    }

    /// The per-game usage and when it was walked: walked again once the stash changed (at most
    /// every `usage_min_age_ms`) and at least every `usage_max_age_ms` (the last-use times),
    /// its names looked up again every [`NAMES_MAX_AGE_MS`].
    fn usage(&self, now_ms: u64) -> (u64, Vec<UsageRow>) {
        let mut u = self.usage.lock().unwrap_or_else(PoisonError::into_inner);
        let changes = self.store.all_changes();
        let fresh = |at: u64, then: u64| {
            let age = now_ms.saturating_sub(at);
            age < self.usage_max_age_ms && (then == changes || age < self.usage_min_age_ms)
        };
        let named = |row: (GameDir, Usage)| {
            let (game, usage) = row;
            let name = self.names.name(&game.service, &game.game);
            let group = self.names.group(&game.service, &game.game);
            UsageRow { game, usage, name, group }
        };
        match &mut *u {
            Some((at, then, v, names_at)) if fresh(*at, *then) => {
                if now_ms.saturating_sub(*names_at) >= NAMES_MAX_AGE_MS {
                    *v = v.drain(..).map(|r| named((r.game, r.usage))).collect();
                    *names_at = now_ms;
                }
                (*at, v.clone())
            }
            _ => {
                let v: Vec<UsageRow> = self.store.usage().into_iter().map(named).collect();
                *u = Some((now_ms, changes, v.clone(), now_ms));
                (now_ms, v)
            }
        }
    }

    /// The stash's entries, as the latest snapshot has them (worked out again only when stale).
    #[must_use]
    pub fn usage_now(&self) -> Vec<UsageRow> {
        self.usage(self.clock.now_ms()).1
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let now_ms = self.clock.now_ms();
        let counters = self.counters();
        let downloads = self.cache.fetcher().map(|f| f.downloads()).unwrap_or_default();
        let (usage_ms, usage) = self.usage(now_ms);
        Snapshot {
            now_ms,
            started_ms: self.started_ms,
            counters,
            downloads,
            usage,
            usage_ms,
            prefill: None,
            prefills: Vec::new(),
            deleting: None,
            builds_changed: Vec::new(),
            clients: Vec::new(),
        }
    }
}
