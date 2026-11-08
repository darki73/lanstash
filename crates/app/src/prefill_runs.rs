//! Use case: the prefill jobs that ran, beyond a restart. Every few seconds
//! each platform's last job is looked at; one that ended and is not kept yet
//! is kept in a [`PrefillLog`]. After a restart, a platform's last job is the
//! last one kept until a new one starts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Pause;
use lanstash_ports::prefill::{Prefill, PrefillLog, PrefillState, PrefillStatus};

/// How often the jobs are looked at.
pub const LOOK_MS: u64 = 5_000;

/// The prefill jobs that ran.
pub struct PrefillRuns {
    jobs: Vec<(String, Arc<dyn Prefill>)>,
    log: Arc<dyn PrefillLog>,
    /// (platform, game, start) of the jobs kept by this run.
    kept: Mutex<BTreeSet<(String, String, u64)>>,
    /// Each platform's last kept job, once asked for (the status asks every second).
    last: Mutex<BTreeMap<String, Option<PrefillStatus>>>,
}

impl std::fmt::Debug for PrefillRuns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefillRuns").finish_non_exhaustive()
    }
}

impl PrefillRuns {
    /// The jobs of each platform (by its id), kept in `log`.
    #[must_use]
    pub fn new(jobs: Vec<(String, Arc<dyn Prefill>)>, log: Arc<dyn PrefillLog>) -> Self {
        Self { jobs, log, kept: Mutex::new(BTreeSet::new()), last: Mutex::new(BTreeMap::new()) }
    }

    /// `platform`'s last kept job (read once, then followed as jobs are kept).
    #[must_use]
    pub fn last(&self, platform: &str) -> Option<PrefillStatus> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        last.entry(platform.to_owned())
            .or_insert_with(|| self.runs(Some(platform), 1).into_iter().next().map(|(_, s)| s))
            .clone()
    }

    /// Keeps each platform's jobs that ended and are not kept yet.
    pub fn look(&self) {
        for (platform, prefill) in &self.jobs {
            let last = prefill.status().filter(|s| s.state != PrefillState::Running);
            for s in prefill.take_ended().into_iter().chain(last) {
                self.keep(platform, s);
            }
        }
    }

    fn keep(&self, platform: &str, s: PrefillStatus) {
        let key = (platform.to_owned(), s.game.clone(), s.started_ms);
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if kept.contains(&key) {
            return;
        }
        match self.log.record(platform, &s) {
            Ok(()) => {
                kept.insert(key);
                self.last.lock().unwrap_or_else(PoisonError::into_inner).insert(platform.to_owned(), Some(s));
            }
            Err(e) => println!("prefill job not kept: {e}"),
        }
    }

    /// The last `limit` jobs kept, of `platform` or of all, newest first, with their platform.
    #[must_use]
    pub fn runs(&self, platform: Option<&str>, limit: usize) -> Vec<(String, PrefillStatus)> {
        self.log.runs(platform, limit).unwrap_or_else(|e| {
            println!("prefill jobs not read: {e}");
            Vec::new()
        })
    }

    /// Looks every [`LOOK_MS`] on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        crate::rounds::forever("prefill-runs", Arc::clone(&pause), move || {
            pause.pause_ms(LOOK_MS);
            me.look();
            crate::rounds::Next::Again
        })
    }
}
