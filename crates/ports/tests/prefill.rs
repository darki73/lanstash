use std::sync::Mutex;

use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, PrefillStatus, StartError};

#[test]
fn states_and_sources_by_name() {
    let names: Vec<&str> = [PrefillState::Running, PrefillState::Done, PrefillState::Stopped, PrefillState::Failed]
        .map(PrefillState::as_str)
        .to_vec();
    assert_eq!(names, ["running", "done", "stopped", "failed"]);
    assert_eq!([PrefillFrom::Cdn, PrefillFrom::Lancache].map(PrefillFrom::as_str), ["cdn", "lancache"]);
    assert_eq!(PrefillFrom::default(), PrefillFrom::Cdn);
    for s in ["running", "done", "stopped", "failed"] {
        assert_eq!(PrefillState::parse(s).map(PrefillState::as_str), Some(s));
    }
    for f in ["cdn", "lancache", "lancache-only"] {
        assert_eq!(PrefillFrom::parse(f).map(PrefillFrom::as_str), Some(f));
    }
    assert_eq!((PrefillState::parse("Done"), PrefillFrom::parse("")), (None, None));
}

#[test]
fn a_started_job_knows_nothing_yet() {
    let s = PrefillStatus::started("osi", true, PrefillFrom::Lancache, 7);
    assert_eq!(
        (s.game.as_str(), s.prerelease, s.from, s.state),
        ("osi", true, PrefillFrom::Lancache, PrefillState::Running)
    );
    assert_eq!((s.files_total, s.files_local, s.bytes, s.started_ms, s.finished_ms), (0, 0, 0, 7, None));
}

/// One job at a time, finished as soon as it starts.
#[derive(Debug, Default)]
struct Instant(Mutex<Option<PrefillStatus>>);

impl Prefill for Instant {
    fn start(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<(), StartError> {
        if game.is_empty() {
            return Err(StartError::NotAGame);
        }
        if from == PrefillFrom::Lancache && !self.lancache() {
            return Err(StartError::NoLancache);
        }
        let s = PrefillStatus {
            state: PrefillState::Done,
            finished_ms: Some(1),
            ..PrefillStatus::started(game, prerelease, from, 1)
        };
        *self.0.lock().unwrap() = Some(s);
        Ok(())
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        self.0.lock().unwrap().clone()
    }
}

#[test]
fn a_job_reports_its_status() {
    let p = Instant::default();
    assert_eq!(p.status(), None);
    assert_eq!(p.start("", false, PrefillFrom::Cdn), Err(StartError::NotAGame));
    assert_eq!(p.start("osi", false, PrefillFrom::Lancache), Err(StartError::NoLancache), "no lancache by default");
    p.start("osi", true, PrefillFrom::Cdn).unwrap();
    p.stop();
    let s = p.status().unwrap();
    assert_eq!((s.game.as_str(), s.prerelease, s.state), ("osi", true, PrefillState::Done));
}
