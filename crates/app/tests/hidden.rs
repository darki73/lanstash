//! Tests for `lanstash_app::hidden`: the games kept private, beyond a restart.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_app::hidden::{HiddenGames, KEPT, is_game_key};
use lanstash_ports::keep::Keep;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keep for Memory {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), data.to_vec());
        Ok(())
    }
}

/// A keep that refuses to save.
struct Full;

impl Keep for Full {
    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
    fn save(&self, _: &str, _: &[u8]) -> Result<(), String> {
        Err("disk full".into())
    }
}

#[test]
fn marks_are_kept_across_a_restart_and_taken_away() {
    let keep = Arc::new(Memory::default());
    let h = HiddenGames::new(keep.clone()).unwrap();
    h.set("steam/123", true).unwrap();
    h.set("blizzard/wow", true).unwrap();
    h.set("steam/123", true).unwrap();
    assert_eq!(h.list(), ["blizzard/wow", "steam/123"]);
    let again = HiddenGames::new(keep.clone()).unwrap();
    assert_eq!(again.list(), ["blizzard/wow", "steam/123"]);
    again.set("steam/123", false).unwrap();
    again.set("steam/999", false).unwrap();
    assert_eq!(again.list(), ["blizzard/wow"]);
    assert_eq!(keep.load(KEPT).unwrap().unwrap(), b"blizzard/wow\n");
}

#[test]
fn only_games_are_marked_and_bad_lines_are_skipped() {
    for bad in
        ["steam", "/123", "steam/", "Steam/1", "steam/1/2", "steam/a b", "steam/..", &format!("s/{}", "x".repeat(65))]
    {
        assert!(!is_game_key(bad), "{bad}");
    }
    assert!(is_game_key("epicgames/Fortnite_Live-1"));
    let keep = Arc::new(Memory::default());
    keep.save(KEPT, b"steam/1\nnot a game\n\n  blizzard/hs  \n").unwrap();
    let h = HiddenGames::new(keep).unwrap();
    assert_eq!(h.list(), ["blizzard/hs", "steam/1"]);
    assert!(h.set("nope", true).is_err());
}

#[test]
fn a_mark_that_could_not_be_kept_changes_nothing() {
    let h = HiddenGames::new(Arc::new(Full)).unwrap();
    assert_eq!(h.set("steam/1", true).unwrap_err(), "disk full");
    assert!(h.list().is_empty());
}
