//! Tests for `lanstash_app::build_history`: what changed is noted, the first
//! status is only taken in, what was seen outlives a restart, and what cannot
//! be kept is noted again.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lanstash_app::build_history::BuildHistory;
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::history::{Change, Event, EventQuery, HistoryLog, Seen, SeenAll};
use lanstash_ports::platform::BuildStatus;

fn st(game: &str, version: &str, known: bool, current: bool) -> BuildStatus {
    BuildStatus { game: game.into(), version: version.into(), known, current }
}

/// The events, oldest first, as (at, game, change, version, from).
fn seen(h: &BuildHistory) -> Vec<(u64, String, Change, String, Option<String>)> {
    let mut e: Vec<_> =
        h.events(None, &[], usize::MAX).into_iter().map(|e| (e.at_ms, e.game, e.change, e.version, e.from)).collect();
    e.reverse();
    e
}

#[test]
fn the_first_status_is_taken_in_then_changes_are_noted() {
    let h = BuildHistory::new(Arc::new(MemoryLogs::default()));
    h.record("steam", &[], 1);
    h.record("steam", &[st("7", "1", true, false), st("8", "5", true, true)], 2);
    assert!(seen(&h).is_empty(), "the games already there are not news");
    h.record("steam", &[st("7", "1", true, true), st("8", "5", true, true)], 3);
    h.record("steam", &[st("7", "2", false, false), st("8", "5", true, true), st("9", "1", true, true)], 4);
    h.record("steam", &[st("7", "2", true, false)], 5);
    h.record("steam", &[st("7", "2", true, true)], 6);
    assert_eq!(
        seen(&h),
        vec![
            (3, "7".into(), Change::Current, "1".into(), None),
            (4, "7".into(), Change::NewBuild, "2".into(), Some("1".into())),
            (4, "9".into(), Change::Added, "1".into(), None),
            (4, "9".into(), Change::Current, "1".into(), None),
            (6, "7".into(), Change::Current, "2".into(), None),
        ]
    );
}

#[test]
fn not_known_when_taken_in_its_first_answer_is_no_news() {
    // Right after a start, Battle.net's plans are not made yet: a game that comes out "current"
    // a minute later has not just become so.
    let h = BuildHistory::new(Arc::new(MemoryLogs::default()));
    h.record("blizzard", &[st("wow", "1", false, false), st("osi", "3", false, false)], 1);
    h.record("blizzard", &[st("wow", "1", true, true), st("osi", "3", true, false)], 2);
    assert!(seen(&h).is_empty(), "{:?}", seen(&h));
    // From there on a change is news.
    h.record("blizzard", &[st("wow", "1", true, true), st("osi", "3", true, true)], 3);
    assert_eq!(seen(&h), vec![(3, "osi".into(), Change::Current, "3".into(), None)]);
}

#[test]
fn a_new_build_not_known_yet_is_news_once_it_is_held() {
    let h = BuildHistory::new(Arc::new(MemoryLogs::default()));
    h.record("steam", &[st("7", "1", true, true)], 1);
    h.record("steam", &[st("7", "2", false, false)], 2);
    h.record("steam", &[st("7", "2", true, true)], 3);
    assert_eq!(
        seen(&h),
        vec![
            (2, "7".into(), Change::NewBuild, "2".into(), Some("1".into())),
            (3, "7".into(), Change::Current, "2".into(), None)
        ]
    );
}

#[test]
fn a_build_not_known_yet_changes_nothing_and_a_game_left_out_is_not_gone() {
    let h = BuildHistory::new(Arc::new(MemoryLogs::default()));
    h.record("blizzard", &[st("wow", "1", true, true)], 1);
    h.record("blizzard", &[st("wow", "1", false, false)], 2);
    h.record("blizzard", &[], 3);
    h.record("blizzard", &[st("wow", "1", true, true)], 4);
    assert!(seen(&h).is_empty(), "{:?}", seen(&h));
}

#[test]
fn what_was_seen_outlives_a_restart() {
    let log = Arc::new(MemoryLogs::default());
    let h = BuildHistory::new(log.clone());
    h.record("steam", &[st("7", "1", true, true)], 1);
    h.record("steam", &[st("7", "2", true, false)], 2);
    let again = BuildHistory::new(log);
    assert_eq!(seen(&again), seen(&h));
    // A new build that came while stopped is noted at the next start; the first status is not taken in again.
    again.record("steam", &[st("7", "3", true, true)], 9);
    assert_eq!(seen(&again).len(), 3);
    assert_eq!(seen(&again)[1], (9, "7".into(), Change::NewBuild, "3".into(), Some("2".into())));
    assert_eq!(seen(&again)[2].2, Change::Current);
}

#[test]
fn one_platforms_and_one_games_events() {
    let h = BuildHistory::new(Arc::new(MemoryLogs::default()));
    for p in ["steam", "blizzard"] {
        h.record(p, &[st("7", "1", true, false)], 1);
        h.record(p, &[st("7", "2", true, false), st("8", "1", true, false)], 2);
    }
    assert_eq!(h.events(Some("steam"), &[], 10).len(), 2);
    assert_eq!(h.events(Some("steam"), &["8".to_string()], 10).len(), 1);
    assert_eq!(h.events(None, &[], 1).len(), 1);
}

/// A log that refuses to keep while `refusing` is set.
#[derive(Default)]
struct Flaky {
    inner: MemoryLogs,
    refusing: AtomicBool,
}

impl HistoryLog for Flaky {
    fn seen(&self) -> Result<SeenAll, String> {
        self.inner.seen()
    }
    fn note(&self, platform: &str, events: &[Event], seen: &[Seen]) -> Result<(), String> {
        if self.refusing.load(Ordering::Relaxed) {
            return Err("disk full".into());
        }
        self.inner.note(platform, events, seen)
    }
    fn events(&self, q: &EventQuery<'_>) -> Result<Vec<Event>, String> {
        self.inner.events(q)
    }
}

#[test]
fn what_could_not_be_kept_is_noted_again() {
    let log = Arc::new(Flaky::default());
    let h = BuildHistory::new(log.clone());
    h.record("steam", &[st("7", "1", true, false)], 1);
    log.refusing.store(true, Ordering::Relaxed);
    h.record("steam", &[st("7", "2", true, false)], 2);
    assert!(seen(&h).is_empty());
    log.refusing.store(false, Ordering::Relaxed);
    h.record("steam", &[st("7", "2", true, false)], 3);
    assert_eq!(seen(&h), vec![(3, "7".into(), Change::NewBuild, "2".into(), Some("1".into()))]);
}
