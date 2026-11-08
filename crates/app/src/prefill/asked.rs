//! A job asked for, and how it gets on the line: started at once when the line is free, queued
//! behind the running job otherwise (a job already running or queued for the same game, build and
//! install is not asked for twice). A job names the install it is for when not the one the
//! platform is set up for; only a platform that prefills other installs takes one.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use lanstash_ports::prefill::{PrefillFrom, PrefillState, PrefillStatus, StartError};

use super::Prefiller;

/// A job asked for: the game, its pre-release build or not, where from, and the install it is for
/// (none: the one the platform is set up for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Asked {
    pub(super) game: String,
    pub(super) prerelease: bool,
    pub(super) from: PrefillFrom,
    pub(super) install: Option<String>,
}

impl Asked {
    pub(super) fn new(game: &str, prerelease: bool, from: PrefillFrom, install: Option<&str>) -> Self {
        Self { game: game.to_owned(), prerelease, from, install: install.map(String::from) }
    }

    /// Its job's status as it starts at `now_ms`.
    pub(super) fn started(&self, now_ms: u64) -> PrefillStatus {
        let mut s = PrefillStatus::started(&self.game, self.prerelease, self.from, now_ms);
        s.install.clone_from(&self.install);
        s
    }
}

impl Prefiller {
    /// Whether `asked` may be asked for at all.
    fn admit(&self, asked: &Asked) -> Result<(), StartError> {
        self.shared.planner.admit(&asked.game)?;
        if asked.from.reads_lancache() && self.shared.lancache.is_none() {
            return Err(StartError::NoLancache);
        }
        if asked.install.is_some() && !self.shared.planner.installs() {
            return Err(StartError::NoInstall);
        }
        Ok(())
    }

    /// Starts `asked` now.
    pub(super) fn begin(&self, asked: Asked) -> Result<(), StartError> {
        self.admit(&asked)?;
        {
            let mut st = self.shared.lock();
            if self.shared.busy(&st) {
                return Err(StartError::Busy);
            }
            self.shared.worker.store(true, Ordering::SeqCst);
            *st = Some(asked.started(self.shared.clock.now_ms()));
        }
        self.shared.stop.store(false, Ordering::SeqCst);
        self.shared.bytes.store(0, Ordering::SeqCst);
        self.shared.local.store(0, Ordering::SeqCst);
        let shared = Arc::clone(&self.shared);
        std::thread::Builder::new()
            .name("lanstash-prefill".into())
            .spawn(move || shared.run_queue(asked))
            .map(|_| ())
            .map_err(|e| {
                self.shared.finish(PrefillState::Failed, Some(e.to_string()));
                let _st = self.shared.lock();
                self.shared.worker.store(false, Ordering::SeqCst);
                StartError::Spawn(e.to_string())
            })
    }

    /// Starts `asked` now, or queues it behind the running job: how many wait before it.
    pub(super) fn enqueue(&self, asked: Asked) -> Result<usize, StartError> {
        self.admit(&asked)?;
        {
            // Under the status lock, so the running job cannot end between the look and the push.
            let st = self.shared.lock();
            if self.shared.busy(&st) {
                let mut q = self.shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
                let same = |game: &str, install: &Option<String>| game == asked.game && *install == asked.install;
                let running = st.as_ref().is_some_and(|s| same(&s.game, &s.install));
                if running || q.iter().any(|a| same(&a.game, &a.install) && a.prerelease == asked.prerelease) {
                    return Ok(q.len());
                }
                q.push_back(asked);
                return Ok(q.len());
            }
        }
        self.begin(asked).map(|()| 0)
    }
}
