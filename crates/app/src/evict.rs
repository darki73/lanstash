//! Use case: keep the store within its limits.
//!
//! Each pass compares the store with `max_size` and every root's file system
//! with `keep_free`, and deletes the least recently used segments and object
//! files until both hold. After deleting, a few passes are skipped: some file
//! systems (network shares, ZFS) report freed space with a delay, and acting on the
//! old number would delete more than needed.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_domain::evict::{Candidate, Limits, choose};
use lanstash_store::evict::{EvictItem, Victim};
use lanstash_store::store::Store;

use lanstash_ports::clock::Clock;
use lanstash_ports::events::{Event, Events};

use crate::once_per::OncePer;

/// Eviction settings.
#[derive(Debug, Clone, Copy)]
pub struct EvictOptions {
    pub limits: Limits,
    /// Items used within this long are never evicted (downloads writing them,
    /// clients reading them).
    pub protect_ms: u64,
    /// Passes within this long after an eviction do nothing.
    pub cooldown_ms: u64,
}

impl Default for EvictOptions {
    /// No limits; 10 minutes of protection, 2 minutes of cooldown.
    fn default() -> Self {
        Self { limits: Limits::default(), protect_ms: 10 * 60_000, cooldown_ms: 2 * 60_000 }
    }
}

counters! {
    /// Totals, plus the latest measurements.
    pub struct EvictCounters {
    pub passes: AtomicU64,
    pub evicted_segments: AtomicU64,
    pub evicted_files: AtomicU64,
    pub evicted_bytes: AtomicU64,
    pub failed: AtomicU64,
    /// Bytes stored at the last pass.
    pub stored_bytes: AtomicU64,
    /// Free bytes of the default root's file system at the last pass.
    pub disk_free_bytes: AtomicU64,
    /// Bytes that should have been freed but nothing evictable was left.
    pub short_bytes: AtomicU64,
    /// Journals rewritten without the entries of evicted object files.
    pub journals_compacted: AtomicU64,
    }
}

/// What one pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvictPass {
    /// Skipped (no limits, or cooling down after an eviction).
    pub skipped: bool,
    /// Bytes over `max_size`.
    pub over_max_size: u64,
    /// Bytes missing to `keep_free`, per root.
    pub short_of_free: Vec<u64>,
    /// Deleted: (victim, bytes freed).
    pub evicted: Vec<(Victim, u64)>,
    /// Could not be deleted: (victim, error).
    pub failed: Vec<(Victim, String)>,
}

pub struct Evictor {
    store: Arc<Store>,
    events: Arc<dyn Events>,
    clock: Arc<dyn Clock>,
    opts: EvictOptions,
    pub counters: EvictCounters,
    last_eviction_ms: AtomicU64,
    /// The bytes stored, by the stash's change count: adding them up walks every object
    /// (with the index locked meanwhile), too costly to repeat on every pass.
    stored: OncePer<u64, u64>,
}

impl std::fmt::Debug for Evictor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Evictor").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl Evictor {
    #[must_use]
    pub fn new(store: Arc<Store>, events: Arc<dyn Events>, clock: Arc<dyn Clock>, opts: EvictOptions) -> Self {
        Self {
            store,
            events,
            clock,
            opts,
            counters: EvictCounters::default(),
            last_eviction_ms: AtomicU64::new(0),
            stored: OncePer::default(),
        }
    }

    #[must_use]
    pub fn options(&self) -> EvictOptions {
        self.opts
    }

    /// One pass: measure, and evict if a limit is exceeded.
    pub fn run_once(&self) -> EvictPass {
        let now = self.clock.now_ms();
        self.counters.passes.fetch_add(1, Ordering::Relaxed);
        let limits = self.opts.limits;
        let stored = *self.stored.get(self.store.all_changes(), || self.store.stored_bytes());
        self.counters.stored_bytes.store(stored, Ordering::Relaxed);
        let roots = self.store.roots();
        let disks: Vec<_> = roots.iter().map(|r| r.space().ok().flatten()).collect();
        if let Some(Some(d)) = disks.first() {
            self.counters.disk_free_bytes.store(d.free, Ordering::Relaxed);
        }
        let last = self.last_eviction_ms.load(Ordering::Relaxed);
        if limits.is_off() || (last > 0 && now.saturating_sub(last) < self.opts.cooldown_ms) {
            return EvictPass { skipped: true, ..EvictPass::default() };
        }
        let mut pass = EvictPass {
            over_max_size: limits.over_max_size(stored),
            short_of_free: disks.iter().map(|d| d.map_or(0, |d| limits.short_of_free(d))).collect(),
            ..EvictPass::default()
        };
        if pass.over_max_size == 0 && pass.short_of_free.iter().all(|&n| n == 0) {
            self.counters.short_bytes.store(0, Ordering::Relaxed);
            return pass;
        }

        let cands = self.store.eviction_candidates();
        let chosen = pick(&cands, &pass, now.saturating_sub(self.opts.protect_ms));
        let chosen_bytes: u64 = chosen.iter().map(|&i| cands[i].bytes).sum();
        let wanted = pass.over_max_size.max(pass.short_of_free.iter().copied().max().unwrap_or(0));
        self.counters.short_bytes.store(wanted.saturating_sub(chosen_bytes), Ordering::Relaxed);
        for i in chosen {
            let v = &cands[i];
            match self.store.evict(v) {
                Ok(bytes) => {
                    let n = match v.item {
                        EvictItem::Segment(_) => &self.counters.evicted_segments,
                        EvictItem::File(_) => &self.counters.evicted_files,
                    };
                    n.fetch_add(1, Ordering::Relaxed);
                    self.counters.evicted_bytes.fetch_add(bytes, Ordering::Relaxed);
                    self.events.emit(&Event::Evicted { path: &v.path, bytes, last_used_ms: v.last_used_ms });
                    pass.evicted.push((v.clone(), bytes));
                }
                Err(e) => {
                    self.counters.failed.fetch_add(1, Ordering::Relaxed);
                    let error = e.to_string();
                    self.events.emit(&Event::EvictFailed { path: &v.path, error: &error });
                    pass.failed.push((v.clone(), error));
                }
            }
        }
        if pass.evicted.iter().any(|(v, _)| matches!(v.item, EvictItem::File(_))) {
            let (done, failed) = self.store.compact_journals();
            self.counters.journals_compacted.fetch_add(done.len() as u64, Ordering::Relaxed);
            for (game, e) in failed {
                let (path, error) = (game.journal(), e.to_string());
                self.events.emit(&Event::EvictFailed { path: &path, error: &error });
            }
        }
        if !pass.evicted.is_empty() {
            self.last_eviction_ms.store(now.max(1), Ordering::Relaxed);
        }
        pass
    }
}

/// The candidates to evict, least recently used first: enough for
/// `max_size` over all roots, then whatever each root still needs for
/// `keep_free` from its own candidates.
fn pick(cands: &[Victim], pass: &EvictPass, protect_since_ms: u64) -> Vec<usize> {
    let as_cand = |v: &Victim| Candidate { bytes: v.bytes, last_used_ms: v.last_used_ms };
    let all: Vec<Candidate> = cands.iter().map(as_cand).collect();
    let mut chosen: BTreeSet<usize> = choose(&all, pass.over_max_size, protect_since_ms).into_iter().collect();
    for (root, &short) in pass.short_of_free.iter().enumerate() {
        let freed: u64 = chosen.iter().filter(|&&i| cands[i].root == root).map(|&i| cands[i].bytes).sum();
        let need = short.saturating_sub(freed);
        if need == 0 {
            continue;
        }
        let here: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].root == root && !chosen.contains(&i)).collect();
        let sub: Vec<Candidate> = here.iter().map(|&i| all[i]).collect();
        chosen.extend(choose(&sub, need, protect_since_ms).into_iter().map(|j| here[j]));
    }
    let mut out: Vec<usize> = chosen.into_iter().collect();
    out.sort_by_key(|&i| cands[i].last_used_ms);
    out
}
