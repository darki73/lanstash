//! Use case: the counters over time, beyond a restart. Once a minute, how much
//! each counter went up since the minute before is added to a [`StatsLog`] (a
//! counter that went down started again from 0: a restart; all of it counts).
//! Gauges (what is stored, free space) are not counters and are left out by
//! name. Minutes older than `keep_ms` are forgotten once an hour.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::stats::{Bucket, StatsLog};

/// The counters as they are now, by name (`group.counter`).
pub type CounterSource = Arc<dyn Fn() -> Vec<(String, u64)> + Send + Sync>;

const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;

/// The most buckets a series has (a longer one is asked in larger buckets).
pub const MAX_BUCKETS: u64 = 2_000;

/// The bucket size a series from `from_ms` to `to_ms` asked in buckets of `step_ms` gets: a whole
/// number of minutes, large enough for at most [`MAX_BUCKETS`].
#[must_use]
pub fn step(from_ms: u64, to_ms: u64, step_ms: u64) -> u64 {
    let span = to_ms.saturating_sub(from_ms);
    step_ms.max(span.div_ceil(MAX_BUCKETS)).div_ceil(MINUTE).max(1) * MINUTE
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptOptions {
    /// How long minutes are kept.
    pub keep_ms: u64,
    /// Names left out: whole (`eviction.stored_bytes`) or a group (`lifetime.*`).
    pub skip: Vec<String>,
}

impl Default for KeptOptions {
    fn default() -> Self {
        Self {
            keep_ms: 180 * 24 * HOUR,
            skip: [
                "lifetime.*",
                "verify.*",
                "storage.*",
                "eviction.stored_bytes",
                "eviction.disk_free_bytes",
                "eviction.short_bytes",
                "https.active",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

/// Keeps the counters by the minute.
pub struct CountersKept {
    source: CounterSource,
    log: Arc<dyn StatsLog>,
    clock: Arc<dyn Clock>,
    opts: KeptOptions,
    last: Mutex<BTreeMap<String, u64>>,
}

impl std::fmt::Debug for CountersKept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CountersKept").field("opts", &self.opts).finish_non_exhaustive()
    }
}

impl CountersKept {
    #[must_use]
    pub fn new(source: CounterSource, log: Arc<dyn StatsLog>, clock: Arc<dyn Clock>, opts: KeptOptions) -> Self {
        Self { source, log, clock, opts, last: Mutex::new(BTreeMap::new()) }
    }

    fn skipped(&self, name: &str) -> bool {
        self.opts.skip.iter().any(|s| match s.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => s == name,
        })
    }

    /// Adds what went up since the last time to the minute that is ending now.
    ///
    /// # Errors
    /// It could not be kept (then it is added the next time).
    pub fn sample(&self) -> Result<(), String> {
        let now = self.clock.now_ms();
        let values: Vec<(String, u64)> = (self.source)().into_iter().filter(|(n, _)| !self.skipped(n)).collect();
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        let deltas: Vec<(String, u64)> = values
            .iter()
            .map(|(n, v)| {
                let before = last.get(n).copied().unwrap_or(0);
                (n.clone(), if *v >= before { v - before } else { *v })
            })
            .collect();
        let minute = now.saturating_sub(1) / MINUTE * MINUTE;
        self.log.add(minute, &deltas)?;
        last.extend(values);
        Ok(())
    }

    /// The counters `names` from `from_ms` to `to_ms` in buckets of [`step`]`(from_ms, to_ms, step_ms)`.
    ///
    /// # Errors
    /// They could not be read.
    pub fn series(&self, names: &[String], from_ms: u64, to_ms: u64, step_ms: u64) -> Result<Vec<Bucket>, String> {
        self.log.series(names, from_ms / MINUTE * MINUTE, to_ms, step(from_ms, to_ms, step_ms))
    }

    /// The `limit` counters named `prefix…` that went up most from `from_ms` to `to_ms`, most first.
    ///
    /// # Errors
    /// They could not be read.
    pub fn top(&self, prefix: &str, from_ms: u64, to_ms: u64, limit: usize) -> Result<Vec<(String, u64)>, String> {
        self.log.top(prefix, from_ms / MINUTE * MINUTE, to_ms, limit)
    }

    /// Samples every minute on a thread of its own, and forgets old minutes once an hour.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        let mut forgot = 0;
        crate::rounds::forever("counters-kept", Arc::clone(&pause), move || {
            // To the next minute's start, so a minute's row is one minute.
            let now = me.clock.now_ms();
            pause.pause_ms(MINUTE - now % MINUTE);
            if let Err(e) = me.sample() {
                println!("counters not kept: {e}");
            }
            let now = me.clock.now_ms();
            if now.saturating_sub(forgot) >= HOUR {
                forgot = now;
                if let Err(e) = me.log.forget_before(now.saturating_sub(me.opts.keep_ms)) {
                    println!("old counters not forgotten: {e}");
                }
            }
            crate::rounds::Next::Again
        })
    }
}
