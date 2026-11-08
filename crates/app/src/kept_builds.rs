//! A platform whose builds are worked out on a thread of its own: the builds view
//! and the build status are what that thread last worked out, so asking for
//! them costs nothing and a slow platform holds back no other.
//!
//! Working them out means asking the stash about every file of every stashed
//! game's current build (seconds, for a large stash). The thread does it again
//! when the platform's part of the stash changes (a download, a prefill, a
//! deletion), resting between runs twice as long as a run took, and at least
//! every `every_ms` while a build is not known yet (backing off, doubling up to
//! `settled_ms`, while runs change nothing), every `settled_ms` once all are (a
//! new build, a library read). The view and the status come from one run
//! ([`Platform::builds_and_status`]). A platform without builds gets no thread. Until the first run is done
//! a request works the builds out itself. When the builds come out different,
//! it notes when, which the status stream tells the web.
//!
//! Everything else is the platform's own.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::account::Account;
use lanstash_ports::art::ArtSource;
use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::{BuildStatus, BuildsAndStatus, ClientOs, Platform, Pruning, SetupFile, View, ViewError};
use lanstash_ports::prefill::Prefill;
use lanstash_store::store::Store;

use crate::build_history::BuildHistory;

/// How often the thread looks at the stash, at the latest and at least it works the builds out again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepOptions {
    /// How often the stash is looked at for a change.
    pub look_ms: u64,
    /// The longest the builds go without being worked out again while one is not known yet
    /// (its plan or chunk list still being made).
    pub every_ms: u64,
    /// The longest the builds go without being worked out again once all are known (a new
    /// build, a library read: nothing the stash tells).
    pub settled_ms: u64,
    /// The shortest rest between two runs.
    pub rest_ms: u64,
}

impl Default for KeepOptions {
    fn default() -> Self {
        Self { look_ms: 2_000, every_ms: 60_000, settled_ms: 300_000, rest_ms: 5_000 }
    }
}

/// The builds view and status as last worked out.
type Worked = BuildsAndStatus;

pub struct KeptBuilds {
    inner: Arc<dyn Platform>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    opts: KeepOptions,
    kept: Mutex<Option<Worked>>,
    changed_ms: AtomicU64,
    history: Option<Arc<BuildHistory>>,
}

impl core::fmt::Debug for KeptBuilds {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeptBuilds").field("inner", &self.inner).field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl KeptBuilds {
    #[must_use]
    pub fn new(inner: Arc<dyn Platform>, store: Arc<Store>, clock: Arc<dyn Clock>, opts: KeepOptions) -> Self {
        Self { inner, store, clock, opts, kept: Mutex::new(None), changed_ms: AtomicU64::new(0), history: None }
    }

    /// Works the builds out now and keeps them; when they differ from the kept ones,
    /// notes when (see [`KeptBuilds::changed_ms`]).
    pub fn refresh(&self) {
        self.work_out();
    }

    /// As [`KeptBuilds::refresh`]; whether the builds came out different.
    fn work_out(&self) -> bool {
        let (view, status) = self.inner.builds_and_status();
        let now = self.clock.now_ms();
        if let (Some(h), Some(s)) = (&self.history, &status) {
            h.record(self.inner.id(), s, now);
        }
        let worked = Some((view, status));
        let mut kept = self.lock();
        if *kept == worked {
            return false;
        }
        *kept = worked;
        self.changed_ms.store(now, Ordering::Relaxed);
        true
    }

    /// Notes in `history` what changed each time the builds are worked out.
    #[must_use]
    pub fn with_history(mut self, history: Arc<BuildHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// When the builds last changed (0: not worked out yet): what tells the web to ask for them again.
    #[must_use]
    pub fn changed_ms(&self) -> u64 {
        self.changed_ms.load(Ordering::Relaxed)
    }

    /// The platform's id.
    #[must_use]
    pub fn platform(&self) -> &str {
        self.inner.id()
    }

    /// How many times what the platform holds in the stash changed: when it moves, the builds may have.
    /// With the platform's own mark ([`Platform::builds_mark`]): its builds can change without
    /// the stash changing (a deleted game forgotten, a game's file lists read).
    fn mark(&self) -> (u64, u64) {
        (self.store.changes(self.inner.id()), self.inner.builds_mark())
    }

    /// Works the builds out again and again on a thread of its own (named after the
    /// platform); a platform with no builds gets no thread.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        // While a build is not known, how long to wait: every_ms after a run that changed
        // something, twice the last wait after one that did not (up to settled_ms). A build
        // that is never worked out (a product nobody installs) costs no run a minute.
        let mut wait = me.opts.every_ms;
        crate::rounds::forever(format!("{}-builds", self.inner.id()), Arc::clone(&pause), move || {
            let (mark, started) = (me.mark(), me.clock.now_ms());
            let changed = me.work_out();
            if !me.has_builds() {
                return crate::rounds::Next::Stop;
            }
            wait = if changed { me.opts.every_ms } else { wait.saturating_mul(2).min(me.opts.settled_ms) };
            let took = me.clock.now_ms().saturating_sub(started);
            pause.pause_ms(me.opts.rest_ms.max(took.saturating_mul(2)));
            let every = if me.settled() { me.opts.settled_ms } else { wait };
            while me.clock.now_ms().saturating_sub(started) < every && me.mark() == mark {
                pause.pause_ms(me.opts.look_ms);
            }
            crate::rounds::Next::Again
        })
    }

    /// Whether the platform has builds at all (as last worked out).
    fn has_builds(&self) -> bool {
        self.lock().as_ref().is_some_and(|(view, status)| view.is_some() || status.is_some())
    }

    /// Whether every stashed game's build is known (none still being worked out).
    fn settled(&self) -> bool {
        self.lock().as_ref().is_some_and(|(_, status)| status.as_ref().is_none_or(|s| s.iter().all(|b| b.known)))
    }

    fn kept(&self) -> Worked {
        if let Some(w) = self.lock().as_ref() {
            return w.clone();
        }
        // Not worked out yet (just started): this request does it.
        self.refresh();
        self.lock().clone().unwrap_or((None, None))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Worked>> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Platform for KeptBuilds {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn names(&self) -> Option<Arc<dyn GameNames>> {
        self.inner.names()
    }
    fn art(&self) -> Option<Arc<dyn ArtSource>> {
        self.inner.art()
    }
    fn builds(&self) -> Option<View> {
        self.kept().0
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        self.kept().1
    }
    fn builds_and_status(&self) -> Worked {
        self.kept()
    }
    fn builds_mark(&self) -> u64 {
        self.inner.builds_mark()
    }
    fn builds_changed_ms(&self) -> Option<u64> {
        // A platform without builds has no mark; one not worked out yet has 0.
        let none = self.lock().as_ref().is_some_and(|(view, status)| view.is_none() && status.is_none());
        (!none).then(|| self.changed_ms())
    }
    fn catalog(&self) -> Option<View> {
        self.inner.catalog()
    }
    fn game(&self, game: &str) -> Option<View> {
        self.inner.game(game)
    }
    fn nginx(&self, game: &str) -> Option<View> {
        self.inner.nginx(game)
    }
    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        self.inner.prefill()
    }
    fn prune(&self, game: &str) -> Option<Result<Pruning, ViewError>> {
        self.inner.prune(game)
    }
    fn parts(&self, game: &str) -> Option<Vec<String>> {
        self.inner.parts(game)
    }
    fn forget(&self, game: &str) {
        self.inner.forget(game);
    }
    fn account(&self) -> Option<Arc<dyn Account>> {
        self.inner.account()
    }
    fn setup(&self, os: ClientOs) -> Option<SetupFile> {
        self.inner.setup(os)
    }
}
