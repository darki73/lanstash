//! Whether a newer lanstash is released: asked of the [`Releases`] port now and then, never
//! while anyone waits. [`ReleaseWatch::newer_than`] answers with what was last learnt and, when
//! that is due again ([`WatchOptions::every_ms`], a failure after [`WatchOptions::retry_ms`]), asks
//! again on a thread of its own. [`version_check`] is `lanstash doctor`'s line for it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_domain::version::newer;
use lanstash_ports::clock::Clock;
use lanstash_ports::releases::Releases;

use crate::health::{Check, Level};

/// How often releases are asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchOptions {
    /// Between two answers.
    pub every_ms: u64,
    /// After a failed ask.
    pub retry_ms: u64,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self { every_ms: 6 * 3_600_000, retry_ms: 3_600_000 }
    }
}

/// When it was last asked, and what was learnt (`Ok(None)`: no release; `Err`: the ask failed).
type Learnt = Option<(u64, Result<Option<String>, String>)>;

struct Inner {
    releases: Arc<dyn Releases>,
    clock: Arc<dyn Clock>,
    opts: WatchOptions,
    learnt: Mutex<Learnt>,
    asking: AtomicBool,
}

impl Inner {
    fn ask(&self) {
        let answer = self.releases.latest();
        *self.learnt.lock().unwrap_or_else(PoisonError::into_inner) = Some((self.clock.now_ms(), answer));
        self.asking.store(false, Ordering::SeqCst);
    }
}

/// The newest release, kept and asked about again now and then.
pub struct ReleaseWatch {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ReleaseWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReleaseWatch").field("opts", &self.inner.opts).finish_non_exhaustive()
    }
}

impl ReleaseWatch {
    #[must_use]
    pub fn new(releases: Arc<dyn Releases>, clock: Arc<dyn Clock>, opts: WatchOptions) -> Self {
        let inner = Inner { releases, clock, opts, learnt: Mutex::new(None), asking: AtomicBool::new(false) };
        Self { inner: Arc::new(inner) }
    }

    /// The newest release when it is newer than `running`, as last learnt; asks again in the
    /// background when that is due.
    #[must_use]
    pub fn newer_than(&self, running: &str) -> Option<String> {
        let learnt = self.inner.learnt.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let now = self.inner.clock.now_ms();
        let due = match &learnt {
            None => true,
            Some((at, Ok(_))) => now.saturating_sub(*at) >= self.inner.opts.every_ms,
            Some((at, Err(_))) => now.saturating_sub(*at) >= self.inner.opts.retry_ms,
        };
        if due && !self.inner.asking.swap(true, Ordering::SeqCst) {
            let inner = Arc::clone(&self.inner);
            let spawned = std::thread::Builder::new().name("lanstash-releases".into()).spawn(move || inner.ask());
            if spawned.is_err() {
                self.inner.asking.store(false, Ordering::SeqCst);
            }
        }
        match learnt {
            Some((_, Ok(Some(v)))) if newer(&v, running) => Some(v),
            _ => None,
        }
    }
}

/// `lanstash doctor`'s line for the version `running`: whether a newer one is released.
#[must_use]
pub fn version_check(running: &str, releases: &dyn Releases) -> Check {
    match releases.latest() {
        Ok(Some(v)) if newer(&v, running) => Check::new(Level::Warn, "version", format!("{running}; {v} is available")),
        Ok(Some(_)) => Check::new(Level::Ok, "version", format!("{running} (the latest)")),
        Ok(None) => Check::new(Level::Ok, "version", format!("{running} (newer releases: not checked)")),
        Err(e) => Check::new(Level::Warn, "version", format!("{running} (could not check for newer: {e})")),
    }
}
