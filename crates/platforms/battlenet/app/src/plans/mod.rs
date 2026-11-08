//! What an install of a Battle.net build downloads, and how much of it the
//! stash holds.
//!
//! A [`Plan`] is made ([`make`]) from the build's files. For a game the stash
//! holds already they are read through the cache ([`lanstash_app::via_cache`]) so
//! they end up stashed like a client's; for any other (a game only looked at: its
//! page, its size) from the CDN alone, so looking at a game does not put it in
//! the stash. A prefill stashes them anyway: a plan's files start with them.
//!
//! 1. the build and CDN configs;
//! 2. every archive's index and the file index;
//! 3. the download manifest, from an archive or loose, and the entries an
//!    install with [`PlanOptions::tags`] takes, plus the files the build
//!    config names itself;
//! 4. where each lies: archive ranges, loose files, or absent from the CDN
//!    (left out).
//!
//! Plans are kept per build (they do not change), and per install when another platform's is
//! asked for ([`BlizzardPlans::plan_for`], [`key`]); [`BlizzardPlans::ready`]
//! and [`BlizzardPlans::ready_for`] make missing ones on a thread of its own,
//! one at a time, so the builds page and a game's page never wait for one. A build is planned once at a time: a second
//! caller waits for the first and takes its plan. The indexes (hundreds for
//! a large game) are read [`PlanOptions::parallel`] at a time over the
//! region's CDN hosts, and [`BlizzardPlans::progress`] says how many are in.
//! [`BlizzardPlans::coverage`] counts the plan's bytes the stash holds
//! ([`held`]), without counting as a use.

mod held;
mod make;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use lanstash_app::serve::RequestHandler;
use lanstash_battlenet_domain::plan::{Coverage, Plan, PlanDetail};
use lanstash_battlenet_domain::tact::Version;
use lanstash_ports::clock::Clock;
use lanstash_ports::prefill::Planning;
use lanstash_ports::upstream::Upstream;
use lanstash_store::store::Store;

/// How plans are made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanOptions {
    /// The region whose CDN hosts are used.
    pub region: String,
    /// The download-manifest tags an install has: platform, architecture,
    /// locale, region, and optional content.
    pub tags: Vec<String>,
    /// Archive files at most this many bytes apart share a request.
    pub gap: u64,
    /// Largest config, index or manifest read.
    pub max_file: usize,
    /// How long a failed plan is kept before it is tried again.
    pub retry_ms: u64,
    /// Plans kept at most (the oldest go first).
    pub keep: usize,
    /// Configs and indexes read at once while planning (at least 1).
    pub parallel: usize,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            region: "us".into(),
            tags: vec!["Windows".into(), "x86_64".into(), "enUS".into(), "US".into()],
            gap: 64 << 10,
            max_file: 64 << 20,
            retry_ms: 60_000,
            keep: 32,
            parallel: 8,
        }
    }
}

type Made = Result<Arc<Plan>, String>;

/// A plan waiting to be made: the product, the build and the install (none: the one set up).
type Pending = (String, Version, Vec<String>);

/// A planning's files read, and of how many.
type Counters = (Arc<AtomicU64>, Arc<AtomicU64>);

/// Ends a build's planning, however it ends: no longer being made, and the
/// callers waiting for it are woken.
struct Ending<'a> {
    inner: &'a Inner,
    key: &'a str,
}

impl Drop for Ending<'_> {
    fn drop(&mut self) {
        lock(&self.inner.making).remove(self.key);
        self.inner.made.notify_all();
    }
}

struct Inner {
    cache: Arc<dyn RequestHandler>,
    store: Arc<Store>,
    upstream: Arc<dyn Upstream>,
    clock: Arc<dyn Clock>,
    opts: PlanOptions,
    /// By build config: when made, and the outcome.
    plans: Mutex<BTreeMap<String, (u64, Made)>>,
    /// Plans waiting for the planning thread, by [`key`].
    pending: Mutex<BTreeMap<String, Pending>>,
    planning: AtomicBool,
    /// Builds being planned now, by build config: files read, of how many (0: not known yet).
    making: Mutex<BTreeMap<String, Counters>>,
    /// Signalled when a build's planning ends.
    made: Condvar,
}

pub struct BlizzardPlans {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for BlizzardPlans {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlizzardPlans").field("opts", &self.inner.opts).finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl BlizzardPlans {
    /// `cache`: the request path clients use; `upstream` reaches the patch service.
    #[must_use]
    pub fn new(
        cache: Arc<dyn RequestHandler>,
        store: Arc<Store>,
        upstream: Arc<dyn Upstream>,
        clock: Arc<dyn Clock>,
        opts: PlanOptions,
    ) -> Self {
        let inner = Inner {
            cache,
            store,
            upstream,
            clock,
            opts,
            plans: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(BTreeMap::new()),
            planning: AtomicBool::new(false),
            making: Mutex::new(BTreeMap::new()),
            made: Condvar::new(),
        };
        Self { inner: Arc::new(inner) }
    }

    /// The plan of `product`'s build `version`, made now if not kept.
    ///
    /// # Errors
    /// Why it could not be made.
    pub fn plan(&self, product: &str, version: &Version) -> Made {
        self.plan_for(product, version, &[])
    }

    /// The plan of `product`'s build `version` for `install`: another platform's tags in place of
    /// those of their kinds (`OSX`, `arm64`); none: the install set up. Kept apart from the others.
    ///
    /// # Errors
    /// Why it could not be made (a tag the build has no install for).
    pub fn plan_for(&self, product: &str, version: &Version, install: &[&str]) -> Made {
        if let Some(made) = self.inner.kept(&key(version, install)) {
            return made;
        }
        self.inner.make_and_keep(product, version, install)
    }

    /// The plan if kept; otherwise `None`, and it is made in the background.
    #[must_use]
    pub fn ready(&self, product: &str, version: &Version) -> Option<Made> {
        self.ready_for(product, version, &[])
    }

    /// [`Self::ready`] for `install` ([`Self::plan_for`]).
    #[must_use]
    pub fn ready_for(&self, product: &str, version: &Version, install: &[&str]) -> Option<Made> {
        let key = key(version, install);
        if let Some(made) = self.inner.kept(&key) {
            return Some(made);
        }
        let install = install.iter().map(|t| (*t).to_owned()).collect();
        lock(&self.inner.pending).insert(key, (product.to_owned(), version.clone(), install));
        if !self.inner.planning.swap(true, Ordering::SeqCst) {
            let inner = Arc::clone(&self.inner);
            let spawned = std::thread::Builder::new().name("lanstash-plan".into()).spawn(move || inner.drain());
            if spawned.is_err() {
                self.inner.planning.store(false, Ordering::SeqCst);
            }
        }
        None
    }

    /// How far the planning of plan `key` ([`key`]) has come, while it is being planned.
    #[must_use]
    pub fn progress(&self, key: &str) -> Option<Planning> {
        let making = lock(&self.inner.making);
        let (done, total) = making.get(key)?;
        Some(Planning { done: done.load(Ordering::Relaxed), total: total.load(Ordering::Relaxed) })
    }

    /// Whether the install set up has `tag` (`Windows`): its plan is the build's own.
    #[must_use]
    pub fn set_up(&self, tag: &str) -> bool {
        self.inner.opts.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    /// The bytes of `plan` the stash holds.
    #[must_use]
    pub fn coverage(&self, plan: &Plan) -> Coverage {
        self.detail(plan).coverage()
    }

    /// How much of `plan` the stash holds, whole files and archive ranges apart.
    #[must_use]
    pub fn detail(&self, plan: &Plan) -> PlanDetail {
        held::detail(&self.inner.store, plan)
    }
}

/// What a plan is kept by: its build, and the install when not the one set up (`<build>+OSX,arm64`).
#[must_use]
pub fn key(version: &Version, install: &[&str]) -> String {
    if install.is_empty() {
        version.build_config.clone()
    } else {
        format!("{}+{}", version.build_config, install.join(","))
    }
}

impl Inner {
    fn kept(&self, key: &str) -> Option<Made> {
        let now = self.clock.now_ms();
        let plans = lock(&self.plans);
        let (at, made) = plans.get(key)?;
        (made.is_ok() || now.saturating_sub(*at) < self.opts.retry_ms).then(|| made.clone())
    }

    /// Makes and keeps a plan; when it is being planned already, waits for that and takes its
    /// outcome instead.
    fn make_and_keep(&self, product: &str, version: &Version, install: &[&str]) -> Made {
        let key = &key(version, install);
        let (done, total) = {
            let mut making = lock(&self.making);
            while making.contains_key(key) {
                making = self.made.wait(making).unwrap_or_else(PoisonError::into_inner);
            }
            if let Some(made) = self.kept(key) {
                return made;
            }
            let entry = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
            making.insert(key.clone(), entry.clone());
            entry
        };
        let _ending = Ending { inner: self, key };
        let made = self.make(product, version, install, (&done, &total)).map(Arc::new);
        let mut plans = lock(&self.plans);
        plans.insert(key.clone(), (self.clock.now_ms(), made.clone()));
        while plans.len() > self.opts.keep.max(1) {
            let oldest = plans.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                plans.remove(&k);
            }
        }
        made
    }

    /// The planning thread: makes pending plans until none are left.
    fn drain(&self) {
        loop {
            let next = lock(&self.pending).pop_first();
            let Some((key, (product, version, install))) = next else {
                self.planning.store(false, Ordering::SeqCst);
                // A build queued between the pop and the store would wait for the next call: take it.
                if lock(&self.pending).is_empty() || self.planning.swap(true, Ordering::SeqCst) {
                    return;
                }
                continue;
            };
            if self.kept(&key).is_none() {
                let install: Vec<&str> = install.iter().map(String::as_str).collect();
                let _ = self.make_and_keep(&product, &version, &install);
            }
        }
    }
}
