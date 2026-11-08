//! What the downloads did, counted: started, joined, finished, failed, stored, and how the origin
//! answered (error statuses, timeouts, the wait for its response head).

use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_ports::upstream::UpstreamError;

use super::Fetcher;

counters! {
    /// Running totals.
    pub struct FetchCounters {
    pub started: AtomicU64,
    /// Requests that read from a download someone else started.
    pub joined: AtomicU64,
    pub completed: AtomicU64,
    pub failed: AtomicU64,
    pub fetched_bytes: AtomicU64,
    pub stored_records: AtomicU64,
    pub stored_extents: AtomicU64,
    pub stored_bytes: AtomicU64,
    pub crc_mismatch: AtomicU64,
    pub store_errors: AtomicU64,
    pub range_ignored: AtomicU64,
    /// Answers of the origin with a client error (4xx) or a server error (5xx).
    pub upstream_4xx: AtomicU64,
    pub upstream_5xx: AtomicU64,
    /// Requests the origin did not connect or answer in time (before or during the body).
    pub timeouts: AtomicU64,
    /// Milliseconds from asking the origin to its response head, added up, and how many heads:
    /// their quotient is the mean time to first byte (with a clock: [`Fetcher::with_clock`]).
    pub answer_wait_ms: AtomicU64,
    pub answers_timed: AtomicU64,
}
}

pub(super) fn inc(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

impl Fetcher {
    /// Counts a failure that was the origin not answering in time.
    pub(super) fn timed_out(&self, e: &UpstreamError) {
        if e.timed_out {
            inc(&self.counters.timeouts);
        }
    }

    /// Counts the origin's answer: an error status, and the wait for it since `asked` (ms).
    pub(super) fn answered(&self, status: u16, asked: Option<u64>) {
        match status {
            400..=499 => inc(&self.counters.upstream_4xx),
            500..=599 => inc(&self.counters.upstream_5xx),
            _ => {}
        }
        if let (Some(clock), Some(asked)) = (&self.clock, asked) {
            let waited = clock.now_ms().saturating_sub(asked);
            self.counters.answer_wait_ms.fetch_add(waited, Ordering::Relaxed);
            inc(&self.counters.answers_timed);
        }
    }
}
