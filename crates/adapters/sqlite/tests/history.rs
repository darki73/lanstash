//! Tests for `lanstash_adapter_sqlite::history`: events and what was seen are
//! kept together, and read back by platform and game, newest first.

use lanstash_adapter_sqlite::db::Database;
use lanstash_ports::history::{Change, Event, EventQuery, HistoryLog, Seen};

fn event(at_ms: u64, platform: &str, game: &str, change: Change) -> Event {
    Event {
        at_ms,
        platform: platform.into(),
        game: game.into(),
        change,
        version: "2".into(),
        from: (change == Change::NewBuild).then(|| "1".into()),
    }
}

fn seen(platform: &str, game: &str, version: &str, held: Option<bool>) -> Seen {
    Seen { platform: platform.into(), game: game.into(), version: version.into(), held }
}

#[test]
fn what_was_seen_is_kept_and_replaced() {
    let db = Database::in_memory().unwrap();
    assert_eq!(db.seen().unwrap().games, vec![]);
    db.note("steam", &[], &[seen("steam", "7", "1", Some(false))]).unwrap();
    db.note("steam", &[], &[seen("steam", "7", "2", Some(true)), seen("steam", "8", "5", None)]).unwrap();
    db.note("blizzard", &[], &[]).unwrap();
    let all = db.seen().unwrap();
    assert_eq!(
        all.games,
        vec![seen("steam", "7", "2", Some(true)), seen("steam", "8", "5", None)],
        "held whole, or not known when taken in"
    );
    let mut platforms = all.platforms;
    platforms.sort();
    assert_eq!(platforms, ["blizzard", "steam"]);
}

#[test]
fn events_newest_first_by_platform_and_game() {
    let db = Database::in_memory().unwrap();
    db.note("steam", &[event(1, "steam", "7", Change::Added), event(2, "steam", "7", Change::NewBuild)], &[]).unwrap();
    db.note("steam", &[event(3, "steam", "8", Change::Current)], &[]).unwrap();
    db.note("blizzard", &[event(4, "blizzard", "7", Change::Current)], &[]).unwrap();
    let all = db.events(&EventQuery { platform: None, games: &[], limit: 10 }).unwrap();
    assert_eq!(all.iter().map(|e| e.at_ms).collect::<Vec<_>>(), [4, 3, 2, 1]);
    assert_eq!(all[2], event(2, "steam", "7", Change::NewBuild));
    let games = ["7".to_string()];
    let seven = db.events(&EventQuery { platform: Some("steam"), games: &games, limit: 10 }).unwrap();
    assert_eq!(seven.iter().map(|e| e.at_ms).collect::<Vec<_>>(), [2, 1]);
    let last = db.events(&EventQuery { platform: Some("steam"), games: &[], limit: 1 }).unwrap();
    assert_eq!(last.iter().map(|e| e.at_ms).collect::<Vec<_>>(), [3]);
}

#[test]
fn only_the_newest_events_are_kept() {
    use lanstash_adapter_sqlite::history::EVENTS_KEPT;
    let db = Database::in_memory().unwrap();
    let many: Vec<Event> = (0..EVENTS_KEPT + 3).map(|t| event(t, "steam", "7", Change::NewBuild)).collect();
    db.note("steam", &many, &[]).unwrap();
    let all = db.events(&EventQuery { platform: None, games: &[], limit: usize::MAX >> 1 }).unwrap();
    assert_eq!(all.len() as u64, EVENTS_KEPT);
    assert_eq!(all.last().map(|e| e.at_ms), Some(3), "the oldest went");
}
