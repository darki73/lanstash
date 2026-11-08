//! Tests for `lanstash_app::delete_game`: one game deleted from the stash as a
//! job (its segments and object files, the one being written included; a game
//! of several directories as one job), with progress; other games stay.

mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lanstash_app::delete_game::{DeleteError, DeleteState, DeleteStatus, GameDeleter};
use lanstash_app::stash_put::StashPut;
use lanstash_domain::object::object_key;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::{Event, Events};
use lanstash_store::index::Lookup;
use lanstash_store::layout::GameDir;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

#[derive(Default)]
struct Seen(Mutex<Vec<String>>);

impl Events for Seen {
    fn emit(&self, e: &Event<'_>) {
        if let Event::Evicted { path, .. } = e {
            self.0.lock().unwrap().push((*path).to_owned());
        }
    }
}

fn stored(s: &Store, target: &str) -> bool {
    matches!(s.lookup(&object_key("blizzard", target)), Lookup::Found(_))
}

fn wait(d: &GameDeleter) -> DeleteStatus {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = d.status().unwrap();
        if s.state != DeleteState::Running || Instant::now() > until {
            return s;
        }
        std::thread::yield_now();
    }
}

#[test]
fn a_game_goes_whole_and_others_stay() {
    let fs = Arc::new(MemFs::default());
    let (s, _) = Store::open(Roots::single(fs), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    let s = Arc::new(s);
    let put = StashPut::new(Arc::clone(&s), Arc::new(support::profiles::Ranged), 100);
    // sc1live: a small file (a record in its open segment) and part of an archive (an object file).
    put.put("/tpr/sc1live/data/aa/bb/small", 0, b"small file", 10, b"", true).unwrap();
    put.put("/tpr/sc1live/data/aa/bb/archive", 0, &[1u8; 50], 1000, b"", true).unwrap();
    put.put("/tpr/hs/data/aa/bb/keep", 0, b"hearthstone", 11, b"", true).unwrap();
    let seen = Arc::new(Seen::default());
    let d = GameDeleter::new(Arc::clone(&s), seen.clone() as Arc<dyn Events>, Arc::new(FixedClock(5)));
    assert!(d.status().is_none());
    d.start("blizzard", "sc1live").unwrap();
    let r = wait(&d);
    assert_eq!((r.platform.as_str(), r.game.as_str(), r.state), ("blizzard", "sc1live", DeleteState::Done));
    assert_eq!((r.items_total, r.items_done, r.failed.len()), (2, 2, 0), "{r:?}");
    assert!(r.bytes_done >= 60 && r.bytes_done == r.bytes_total, "{r:?}");
    assert_eq!((r.started_ms, r.finished_ms), (5, Some(5)));
    assert!(!stored(&s, "/tpr/sc1live/data/aa/bb/small") && !stored(&s, "/tpr/sc1live/data/aa/bb/archive"));
    assert!(stored(&s, "/tpr/hs/data/aa/bb/keep"), "another game stays");
    assert_eq!(seen.0.lock().unwrap().len(), 2, "each deletion announced");
    let usage = s.usage();
    assert!(usage.get(&GameDir::new("blizzard", "sc1live")).is_none_or(|u| u.bytes == 0), "{usage:?}");
    // Again: nothing left; the game can be stored again.
    d.start("blizzard", "sc1live").unwrap();
    assert_eq!(wait(&d).bytes_done, 0);
    put.put("/tpr/sc1live/data/aa/bb/small", 0, b"small file", 10, b"", true).unwrap();
    assert!(stored(&s, "/tpr/sc1live/data/aa/bb/small"));
}

/// Holds the deletion at its first item until let go.
struct Gate(Mutex<bool>, std::sync::Condvar);

impl Events for Gate {
    fn emit(&self, e: &Event<'_>) {
        if matches!(e, Event::Evicted { .. }) {
            let open = self.0.lock().unwrap();
            drop(self.1.wait_while(open, |open| !*open).unwrap());
        }
    }
}

#[test]
fn a_game_is_told_as_being_deleted_while_it_goes() {
    let (s, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    let s = Arc::new(s);
    let put = StashPut::new(Arc::clone(&s), Arc::new(support::profiles::Ranged), 100);
    put.put("/tpr/sc1live/data/aa/bb/small", 0, b"small file", 10, b"", true).unwrap();
    let gate = Arc::new(Gate(Mutex::new(false), std::sync::Condvar::new()));
    let d = GameDeleter::new(s, gate.clone() as Arc<dyn Events>, Arc::new(FixedClock(1)));
    d.start("blizzard", "sc1live").unwrap();
    assert!(d.deleting("blizzard", "sc1live"));
    assert!(!d.deleting("blizzard", "hs") && !d.deleting("steam", "sc1live"));
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    wait(&d);
    assert!(!d.deleting("blizzard", "sc1live"), "done");
}

#[test]
fn one_at_a_time() {
    let (s, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    let d = GameDeleter::new(Arc::new(s), Arc::new(Seen::default()), Arc::new(FixedClock(1)));
    d.start("blizzard", "a").unwrap();
    // The second start either finds the first still running, or done already.
    match d.start("blizzard", "b") {
        Err(DeleteError::Busy) | Ok(()) => {}
        Err(e) => panic!("{e:?}"),
    }
    assert_eq!(wait(&d).state, DeleteState::Done);
}

#[test]
fn a_game_of_several_directories_goes_as_one_deletion() {
    let fs = Arc::new(MemFs::default());
    let (s, _) = Store::open(Roots::single(fs), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    let s = Arc::new(s);
    let put = StashPut::new(Arc::clone(&s), Arc::new(support::profiles::Ranged), 100);
    put.put("/tpr/d1/data/aa/bb/x", 0, b"depot one", 9, b"", true).unwrap();
    put.put("/tpr/d2/data/aa/bb/y", 0, b"depot two", 9, b"", true).unwrap();
    put.put("/tpr/other/data/aa/bb/z", 0, b"stays", 5, b"", true).unwrap();
    let d = GameDeleter::new(Arc::clone(&s), Arc::new(Seen::default()) as Arc<dyn Events>, Arc::new(FixedClock(5)));
    d.start_parts("blizzard", "app", vec!["d1".into(), "d2".into(), "absent".into()]).unwrap();
    let r = wait(&d);
    assert_eq!((r.game.as_str(), r.state, r.failed.len()), ("app", DeleteState::Done, 0));
    assert!(!stored(&s, "/tpr/d1/data/aa/bb/x") && !stored(&s, "/tpr/d2/data/aa/bb/y"));
    assert!(stored(&s, "/tpr/other/data/aa/bb/z"));
}

#[test]
fn chosen_segments_go_and_the_rest_of_the_game_stays() {
    use lanstash_ports::platform::PruneItem;
    use lanstash_store::evict::EvictItem;
    let fs = Arc::new(MemFs::default());
    let (s, _) = Store::open(Roots::single(fs), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    let s = Arc::new(s);
    let put = StashPut::new(Arc::clone(&s), Arc::new(support::profiles::Ranged), 100);
    put.put("/tpr/g/data/aa/bb/old", 0, b"old build", 9, b"", true).unwrap();
    s.close_segment_of(&GameDir::new("blizzard", "g"));
    put.put("/tpr/g/data/aa/bb/new", 0, b"new build", 9, b"", true).unwrap();
    s.close_segment_of(&GameDir::new("blizzard", "g"));
    // The segment holding the old object: the first one of the game.
    let first = s
        .eviction_candidates()
        .into_iter()
        .filter_map(|v| match v.item {
            EvictItem::Segment(seq) if v.game.game == "g" => Some(seq),
            _ => None,
        })
        .min()
        .unwrap();
    let d = GameDeleter::new(Arc::clone(&s), Arc::new(Seen::default()) as Arc<dyn Events>, Arc::new(FixedClock(5)));
    d.start_items("blizzard", "g", vec![PruneItem::Segment { dir: "g".into(), seq: first }]).unwrap();
    let r = wait(&d);
    assert_eq!((r.state, r.partial, r.items_done), (DeleteState::Done, true, 1));
    assert!(!stored(&s, "/tpr/g/data/aa/bb/old") && stored(&s, "/tpr/g/data/aa/bb/new"));
}
