//! The wall clock and waiting as ports (only the composition root reads the
//! real clock or sleeps).

/// Wall-clock time.
pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
}

/// A clock that always reports the same instant (tests, one import run).
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

/// Waiting (only the composition root sleeps for real).
pub trait Pause: Send + Sync {
    /// Returns after about `ms` milliseconds.
    fn pause_ms(&self, ms: u64);
}

/// A pause that returns at once (tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoPause;

impl Pause for NoPause {
    fn pause_ms(&self, _ms: u64) {}
}
