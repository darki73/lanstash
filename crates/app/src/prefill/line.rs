//! The job line: one job at a time, the queued ones after it on the same thread, and the
//! status each job leaves.

use std::sync::atomic::Ordering;
use std::sync::{MutexGuard, PoisonError};

use lanstash_ports::prefill::{PrefillState, PrefillStatus};

use super::{Asked, Shared};

/// Ended jobs kept until they are asked for.
const ENDED_KEPT: usize = 64;

impl Shared {
    /// Whether the job line is taken: a job runs, or the thread is between two jobs.
    pub(super) fn busy(&self, st: &Option<PrefillStatus>) -> bool {
        self.worker.load(Ordering::SeqCst) || st.as_ref().is_some_and(|s| s.state == PrefillState::Running)
    }

    /// Runs `asked`, then each queued job in turn, until the queue is empty or a stop.
    pub(super) fn run_queue(&self, asked: Asked) {
        let mut asked = asked;
        loop {
            self.run(&asked.game, asked.prerelease, asked.from, asked.install.as_deref());
            // The next job, or the line given up, in one hold of the status lock (see `queue`).
            let mut st = self.lock();
            let next = if self.stop.load(Ordering::SeqCst) {
                None
            } else {
                self.queue.lock().unwrap_or_else(PoisonError::into_inner).pop_front()
            };
            let Some(next) = next else {
                self.worker.store(false, Ordering::SeqCst);
                return;
            };
            *st = Some(next.started(self.clock.now_ms()));
            drop(st);
            self.bytes.store(0, Ordering::SeqCst);
            self.local.store(0, Ordering::SeqCst);
            asked = next;
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, Option<PrefillStatus>> {
        self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn update(&self, f: impl FnOnce(&mut PrefillStatus)) {
        if let Some(s) = self.lock().as_mut() {
            f(s);
        }
    }

    pub(super) fn finish(&self, state: PrefillState, error: Option<String>) {
        let (bytes, local, now) =
            (self.bytes.load(Ordering::Relaxed), self.local.load(Ordering::Relaxed), self.clock.now_ms());
        let mut st = self.lock();
        let Some(s) = st.as_mut() else { return };
        s.state = state;
        s.error = error;
        s.bytes = bytes;
        s.bytes_local = local;
        s.current = None;
        s.finished_ms = Some(now);
        let mut ended = self.ended.lock().unwrap_or_else(PoisonError::into_inner);
        if ended.len() == ENDED_KEPT {
            ended.pop_front();
        }
        ended.push_back(s.clone());
    }
}
