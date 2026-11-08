//! Counters over time, kept beyond a restart: how much each counter went up in
//! each minute (a counter of this run restarts at 0; what it went up by does not).

use alloc::string::String;
use alloc::vec::Vec;

/// One bucket of a series: its start (ms) and, per name asked for, the sum of the bucket's minutes.
pub type Bucket = (u64, Vec<u64>);

/// The kept counters.
pub trait StatsLog: Send + Sync {
    /// Adds `deltas` (counter name, how much it went up) to the minute starting at `minute_ms`.
    ///
    /// # Errors
    /// They could not be kept.
    fn add(&self, minute_ms: u64, deltas: &[(String, u64)]) -> Result<(), String>;

    /// The counters `names` from `from_ms` to before `to_ms`, summed in buckets of `step_ms`
    /// (a multiple of a minute), oldest first; a bucket with nothing kept is left out.
    ///
    /// # Errors
    /// They could not be read.
    fn series(&self, names: &[String], from_ms: u64, to_ms: u64, step_ms: u64) -> Result<Vec<Bucket>, String>;

    /// The `limit` counters whose names start with `prefix` that went up most from `from_ms`
    /// to before `to_ms`, most first, with how much.
    ///
    /// # Errors
    /// They could not be read.
    fn top(&self, prefix: &str, from_ms: u64, to_ms: u64, limit: usize) -> Result<Vec<(String, u64)>, String>;

    /// Forgets the minutes before `before_ms`; how many rows went.
    ///
    /// # Errors
    /// They could not be forgotten.
    fn forget_before(&self, before_ms: u64) -> Result<u64, String>;
}
