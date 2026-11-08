//! Tests for `lanstash_adapter_sqlite::runs`: a job that ended is kept whole,
//! replaced by platform, game and start, and read back newest first.

use lanstash_adapter_sqlite::db::Database;
use lanstash_ports::prefill::{PrefillFrom, PrefillLog, PrefillState, PrefillStatus};

fn ended(game: &str, started_ms: u64, state: PrefillState) -> PrefillStatus {
    let mut s = PrefillStatus::started(game, false, PrefillFrom::Cdn, started_ms);
    s.state = state;
    s.version = Some("v2".into());
    s.files_total = 10;
    s.files_done = 10;
    s.files_skipped = 3;
    s.bytes_skipped = 300;
    s.bytes_total = 1_000;
    s.bytes = 700;
    s.finished_ms = Some(started_ms + 50);
    s.error = (state == PrefillState::Failed).then(|| "the CDN said no".into());
    s
}

#[test]
fn ended_jobs_are_kept_whole_and_read_newest_first() {
    let db = Database::in_memory().unwrap();
    db.record("steam", &ended("7", 100, PrefillState::Done)).unwrap();
    db.record("blizzard", &ended("osi", 200, PrefillState::Failed)).unwrap();
    db.record("steam", &ended("8", 300, PrefillState::Stopped)).unwrap();
    let all = db.runs(None, 10).unwrap();
    assert_eq!(
        all.iter().map(|(p, s)| (p.as_str(), s.game.as_str())).collect::<Vec<_>>(),
        [("steam", "8"), ("blizzard", "osi"), ("steam", "7")]
    );
    assert_eq!(all[1].1, ended("osi", 200, PrefillState::Failed));
    let steam = db.runs(Some("steam"), 1).unwrap();
    assert_eq!(steam.iter().map(|(_, s)| s.game.as_str()).collect::<Vec<_>>(), ["8"]);
}

#[test]
fn only_the_newest_jobs_are_kept() {
    use lanstash_adapter_sqlite::runs::RUNS_KEPT;
    let db = Database::in_memory().unwrap();
    for t in 0..RUNS_KEPT + 5 {
        db.record("steam", &ended(&t.to_string(), t, PrefillState::Done)).unwrap();
    }
    let all = db.runs(None, usize::MAX >> 1).unwrap();
    assert_eq!(all.len() as u64, RUNS_KEPT);
    assert_eq!(all.last().map(|(_, s)| s.started_ms), Some(5), "the oldest went");
}

#[test]
fn the_same_job_kept_again_replaces_it() {
    let db = Database::in_memory().unwrap();
    db.record("steam", &ended("7", 100, PrefillState::Stopped)).unwrap();
    db.record("steam", &ended("7", 100, PrefillState::Done)).unwrap();
    let all = db.runs(None, 10).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1.state, PrefillState::Done);
}
