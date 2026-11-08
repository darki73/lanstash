//! Use case: how much of a Battle.net game's current build an nginx (lancache)
//! cache holds, found from the build's plan rather than by walking the cache.
//!
//! Every slice the plan needs has a known key and so a known file name
//! ([`lanstash_battlenet_domain::nginx_keys`]): the probe only asks whether
//! those files exist. It reads nothing else and writes nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use lanstash_battlenet_domain::nginx_keys::{Tally, tally, wanted};
use lanstash_domain::nginx_cache::cache_file_path;
use lanstash_ports::nginx::NginxCacheSource;

use crate::builds::BlizzardBuilds;

/// Probe settings (passed in from the composition root).
#[derive(Debug, Clone)]
pub struct ProbeOptions {
    /// The cache identifier nginx used for Battle.net (lancache: `blizzard`).
    pub service: String,
    /// nginx `slice` size (lancache: 1 MiB).
    pub slice_bytes: u64,
    /// Files checked at once.
    pub threads: usize,
    /// Missing slices named in the report, at most.
    pub samples: usize,
}

impl Default for ProbeOptions {
    fn default() -> Self {
        Self { service: "blizzard".into(), slice_bytes: 1 << 20, threads: 16, samples: 10 }
    }
}

/// What an nginx cache holds of one build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub game: String,
    pub product: String,
    pub version: String,
    pub whole: Tally,
    pub archived: Tally,
    /// Distinct slice files the build needs, and those present.
    pub slices: u64,
    pub slices_found: u64,
    /// Some missing slices: (nginx key, cache file).
    pub missing: Vec<(String, String)>,
}

/// How much of a build the nginx cache holds, in bytes (whole files and files
/// inside archives held whole).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share {
    pub have: u64,
    pub want: u64,
}

/// Answers "how much of this game does the nginx cache hold".
pub struct NginxProbe {
    builds: Arc<BlizzardBuilds>,
    source: Arc<dyn NginxCacheSource>,
    opts: ProbeOptions,
    /// Shares found, by build config (kept: the probe takes lancache's cache as not changing).
    shares: Mutex<BTreeMap<String, Option<Share>>>,
    /// Games waiting for the background probe, and whether it runs.
    waiting: Mutex<BTreeSet<String>>,
    probing: AtomicBool,
}

impl std::fmt::Debug for NginxProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NginxProbe").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl NginxProbe {
    #[must_use]
    pub fn new(builds: Arc<BlizzardBuilds>, source: Arc<dyn NginxCacheSource>, opts: ProbeOptions) -> Self {
        let (shares, waiting) = (Mutex::new(BTreeMap::new()), Mutex::new(BTreeSet::new()));
        Self { builds, source, opts, shares, waiting, probing: AtomicBool::new(false) }
    }

    /// How much of `game`'s build `build_config` the nginx cache holds, once
    /// probed; `None` meanwhile, and the probe is queued (one game at a time,
    /// on a thread of its own; a probe that fails gives no share).
    pub fn share(self: &Arc<Self>, game: &str, build_config: &str) -> Option<Share> {
        if let Some(found) = lock(&self.shares).get(build_config) {
            return *found;
        }
        lock(&self.waiting).insert(game.to_owned());
        if !self.probing.swap(true, Ordering::SeqCst) {
            let me = Arc::clone(self);
            let spawned = std::thread::Builder::new().name("lanstash-probe".into()).spawn(move || me.drain());
            if spawned.is_err() {
                self.probing.store(false, Ordering::SeqCst);
            }
        }
        None
    }

    /// The background probe: every waiting game, one after another.
    fn drain(&self) {
        loop {
            let next = lock(&self.waiting).pop_first();
            let Some(game) = next else {
                self.probing.store(false, Ordering::SeqCst);
                if lock(&self.waiting).is_empty() || self.probing.swap(true, Ordering::SeqCst) {
                    return;
                }
                continue;
            };
            let Some(config) = self.builds.check(&game).build.map(|b| b.version.build_config) else { continue };
            if lock(&self.shares).contains_key(&config) {
                continue;
            }
            let share = self.probe(&game).ok().map(|r| Share {
                have: r.whole.bytes_found + r.archived.bytes_found,
                want: r.whole.bytes + r.archived.bytes,
            });
            lock(&self.shares).insert(config, share);
        }
    }

    /// Probes the game's current build (planning it first when needed).
    ///
    /// # Errors
    /// The game or its build is unknown, it cannot be planned, or the nginx
    /// cache cannot be read.
    pub fn probe(&self, game: &str) -> Result<ProbeReport, String> {
        let check = self.builds.check(game);
        let (Some(product), Some(build)) = (check.product, check.build) else {
            return Err(check.error.unwrap_or_else(|| format!("{game}: not a product the patch service knows")));
        };
        let plans = self.builds.plans().ok_or("no plans")?;
        let plan = plans.plan(&product, &build.version)?;
        let w = wanted(&plan, &self.opts.service, self.opts.slice_bytes);
        let present = self.present(&w.keys)?;
        let missing = w
            .keys
            .iter()
            .zip(&present)
            .filter(|(_, p)| !**p)
            .take(self.opts.samples)
            .map(|(k, _)| (k.clone(), cache_file_path(k)))
            .collect();
        Ok(ProbeReport {
            game: game.into(),
            product,
            version: build.version.name.clone(),
            whole: tally(&w.whole, &present),
            archived: tally(&w.archived, &present),
            slices: w.keys.len() as u64,
            slices_found: present.iter().filter(|p| **p).count() as u64,
            missing,
        })
    }

    /// Which of the keys' cache files exist, checked in parallel.
    fn present(&self, keys: &[String]) -> Result<Vec<bool>, String> {
        let found: Vec<AtomicBool> = keys.iter().map(|_| AtomicBool::new(false)).collect();
        let next = AtomicUsize::new(0);
        let failed: Mutex<Option<String>> = Mutex::new(None);
        std::thread::scope(|s| {
            for _ in 0..self.opts.threads.clamp(1, 64) {
                s.spawn(|| {
                    while failed.lock().unwrap_or_else(PoisonError::into_inner).is_none() {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(key) = keys.get(i) else { return };
                        match self.source.exists(&cache_file_path(key)) {
                            Ok(b) => found[i].store(b, Ordering::Relaxed),
                            Err(e) => {
                                failed.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert(e.to_string());
                            }
                        }
                    }
                });
            }
        });
        match failed.into_inner().unwrap_or_else(PoisonError::into_inner) {
            Some(e) => Err(format!("reading the nginx cache: {e}")),
            None => Ok(found.into_iter().map(AtomicBool::into_inner).collect()),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
