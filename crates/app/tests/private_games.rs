//! Tests for `lanstash_app::private_games`: each account's private games, beyond a restart.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_app::private_games::{KEPT, PrivateGames};
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
fn each_accounts_marks_are_kept_across_a_restart() {
    let keep = Arc::new(Memory::default());
    let p = PrivateGames::new(keep.clone()).unwrap();
    p.set("player.one", "steam/1", true).unwrap();
    p.set("player.two", "steam/1", true).unwrap();
    p.set("player.one", "blizzard/hs", true).unwrap();
    assert_eq!(p.of("player.one"), ["blizzard/hs", "steam/1"]);
    let again = PrivateGames::new(keep.clone()).unwrap();
    assert_eq!(again.marks(), p.marks());
    assert_eq!(again.marks()["steam/1"].len(), 2);
    again.set("player.two", "steam/1", false).unwrap();
    assert_eq!(again.marks()["steam/1"].iter().collect::<Vec<_>>(), ["player.one"], "per account");
    again.forget("player.one").unwrap();
    assert!(again.marks().is_empty(), "an account's marks go with it");
    assert_eq!(keep.load(KEPT).unwrap().unwrap(), b"");
}

#[test]
fn a_bad_name_game_or_line_is_refused_or_skipped_and_a_failed_keep_changes_nothing() {
    let keep = Arc::new(Memory::default());
    keep.save(KEPT, b"player.one steam/1\nNOT A NAME steam/2\nplayer.two nothing\n").unwrap();
    let p = PrivateGames::new(keep).unwrap();
    assert_eq!(p.marks().keys().collect::<Vec<_>>(), ["steam/1"]);
    assert!(p.set("Player One", "steam/1", true).is_err());
    assert!(p.set("player.one", "steam", true).is_err());
    let full = PrivateGames::new(Arc::new(Full)).unwrap();
    assert!(full.set("player.one", "steam/1", true).is_err());
    assert!(full.marks().is_empty());
}
