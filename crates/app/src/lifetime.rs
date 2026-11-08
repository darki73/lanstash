//! Use case: what the cache did over its whole life, not only since this
//! start: the bytes clients got from the stash (each of which the internet
//! did not have to send) and the bytes fetched from the internet (downloads,
//! prefills, pass-through), since the first start that counted them. The
//! totals of earlier runs are kept in [`Keep`] and this run's counters are
//! added on top; they are saved every minute (and when asked), so a crash
//! loses at most that much.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Pause;
use lanstash_ports::keep::Keep;

use crate::serve::Cache;

/// The name the totals are kept under.
pub const KEPT: &str = "lifetime";

/// The totals of the runs before this one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Before {
    served: u64,
    fetched: u64,
    since_ms: u64,
}

impl Before {
    fn parse(text: &str) -> Self {
        let mut b = Self::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let v = v.trim().parse().unwrap_or(0);
            match k.trim() {
                "served" => b.served = v,
                "fetched" => b.fetched = v,
                "since_ms" => b.since_ms = v,
                _ => {}
            }
        }
        b
    }
}

/// The cache's lifetime totals.
pub struct Lifetime {
    cache: Arc<Cache>,
    keep: Arc<dyn Keep>,
    before: Mutex<Before>,
}

impl std::fmt::Debug for Lifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lifetime").field("before", &self.before).finish_non_exhaustive()
    }
}

impl Lifetime {
    /// The totals of `cache`, on top of those kept in `keep` (counted from `now_ms` when none are).
    #[must_use]
    pub fn new(cache: Arc<Cache>, keep: Arc<dyn Keep>, now_ms: u64) -> Self {
        let mut before =
            keep.load(KEPT).ok().flatten().map(|b| Before::parse(&String::from_utf8_lossy(&b))).unwrap_or_default();
        if before.since_ms == 0 {
            before.since_ms = now_ms;
        }
        Self { cache, keep, before: Mutex::new(before) }
    }

    /// This run's bytes: served from the stash, fetched from the internet.
    fn now(&self) -> (u64, u64) {
        let served = self.cache.counters.hit_bytes.load(Ordering::Relaxed);
        let fetched = self.cache.fetcher().map_or(0, |f| f.counters.fetched_bytes.load(Ordering::Relaxed))
            + self.cache.passthrough().map_or(0, |p| p.counters.bytes.load(Ordering::Relaxed));
        (served, fetched)
    }

    /// The totals, by name: `served_bytes`, `fetched_bytes`, `since_ms`.
    #[must_use]
    pub fn values(&self) -> Vec<(&'static str, u64)> {
        let b = *self.before.lock().unwrap_or_else(PoisonError::into_inner);
        let (served, fetched) = self.now();
        vec![("served_bytes", b.served + served), ("fetched_bytes", b.fetched + fetched), ("since_ms", b.since_ms)]
    }

    /// Keeps the totals as they are now.
    ///
    /// # Errors
    /// They could not be written.
    pub fn save(&self) -> Result<(), String> {
        let b = *self.before.lock().unwrap_or_else(PoisonError::into_inner);
        let (served, fetched) = self.now();
        let text = format!("served={}\nfetched={}\nsince_ms={}\n", b.served + served, b.fetched + fetched, b.since_ms);
        self.keep.save(KEPT, text.as_bytes())
    }

    /// Saves the totals every `every_ms` on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(self: &Arc<Self>, pause: Arc<dyn Pause>, every_ms: u64) -> Result<(), String> {
        let me = Arc::clone(self);
        crate::rounds::forever("lifetime", Arc::clone(&pause), move || {
            pause.pause_ms(every_ms);
            if let Err(e) = me.save() {
                println!("lifetime totals: {e}");
            }
            crate::rounds::Next::Again
        })
    }
}
