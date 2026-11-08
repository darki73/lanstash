//! Tests for `lanstash_app::prefill_runs`: a job is kept once it ends, once,
//! and after a restart a platform's last job is the last one kept.

use std::sync::{Arc, Mutex};

use lanstash_app::prefill_runs::PrefillRuns;
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillLog, PrefillState, PrefillStatus, StartError};

#[derive(Debug, Default)]
struct Job(Mutex<Option<PrefillStatus>>);

impl Prefill for Job {
    fn start(&self, _: &str, _: bool, _: PrefillFrom) -> Result<(), StartError> {
        Ok(())
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        self.0.lock().unwrap().clone()
    }
}

fn job(game: &str, started_ms: u64, state: PrefillState) -> PrefillStatus {
    let mut s = PrefillStatus::started(game, false, PrefillFrom::Cdn, started_ms);
    s.state = state;
    s
}

#[test]
fn a_job_is_kept_once_when_it_ends() {
    let log = Arc::new(MemoryLogs::default());
    let steam = Arc::new(Job::default());
    let runs = PrefillRuns::new(vec![("steam".into(), steam.clone() as Arc<dyn Prefill>)], log.clone());
    runs.look();
    *steam.0.lock().unwrap() = Some(job("7", 1, PrefillState::Running));
    runs.look();
    assert!(runs.runs(None, 10).is_empty(), "a running job is not kept");
    *steam.0.lock().unwrap() = Some(job("7", 1, PrefillState::Done));
    runs.look();
    runs.look();
    assert_eq!(runs.runs(None, 10).len(), 1);
    assert_eq!(runs.last("steam").map(|s| s.state), Some(PrefillState::Done));
    *steam.0.lock().unwrap() = Some(job("8", 2, PrefillState::Failed));
    runs.look();
    assert_eq!(runs.runs(Some("steam"), 10).iter().map(|(_, s)| s.game.as_str()).collect::<Vec<_>>(), ["8", "7"]);
    assert_eq!(runs.last("steam").map(|s| s.game), Some("8".into()), "followed as jobs are kept");
}

/// Jobs that ended between two looks, told by [`Prefill::take_ended`].
#[derive(Debug, Default)]
struct Queue(Mutex<Vec<PrefillStatus>>);

impl Prefill for Queue {
    fn start(&self, _: &str, _: bool, _: PrefillFrom) -> Result<(), StartError> {
        Ok(())
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        self.0.lock().unwrap().last().cloned()
    }
    fn take_ended(&self) -> Vec<PrefillStatus> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[test]
fn every_queued_job_is_kept_though_they_end_between_two_looks() {
    let log = Arc::new(MemoryLogs::default());
    let steam = Arc::new(Queue::default());
    let runs = PrefillRuns::new(vec![("steam".into(), steam.clone() as Arc<dyn Prefill>)], log);
    *steam.0.lock().unwrap() = vec![job("1", 1, PrefillState::Done), job("2", 1, PrefillState::Failed)];
    runs.look();
    runs.look();
    assert_eq!(runs.runs(None, 10).len(), 2);
    assert_eq!(runs.last("steam").map(|s| s.game), Some("2".into()));
}

#[test]
fn after_a_restart_the_last_job_is_the_last_kept() {
    let log = Arc::new(MemoryLogs::default());
    log.record("blizzard", &job("osi", 5, PrefillState::Stopped)).unwrap();
    let runs = PrefillRuns::new(vec![("blizzard".into(), Arc::new(Job::default()) as Arc<dyn Prefill>)], log);
    assert_eq!(runs.last("blizzard").map(|s| (s.game, s.state)), Some(("osi".into(), PrefillState::Stopped)));
    assert_eq!(runs.last("steam"), None);
}
