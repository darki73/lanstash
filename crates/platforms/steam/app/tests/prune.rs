//! Tests for `lanstash_steam_app::prune`: the scripted account's app 100
//! (current chunks 01 and 02 of depot 101, 03 of depot 102), a store with an
//! old build's chunk in a segment of its own, and the current ones in another.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_app::prune_segments::view;
use lanstash_app::stash_put::StashPut;
use lanstash_ports::clock::{Clock, FixedClock};
use lanstash_ports::platform::PruneItem;
use lanstash_steam_app::library::SteamLibrary;
use lanstash_steam_app::prune::SteamPruner;
use lanstash_steam_domain::profile::Steam;
use lanstash_store::layout::GameDir;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use support::account::services;

struct Tick(AtomicU64);

impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn put(put: &StashPut, depot: u32, sha: u8, size: usize) {
    let target = format!("/depot/{depot}/chunk/{}", format!("{sha:02x}").repeat(20));
    put.put(&target, 0, &vec![sha; size], size as u64, &[], true).unwrap();
}

#[test]
fn a_segment_of_old_chunks_goes_after_the_grace_period_and_current_ones_stay() {
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let (st, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let st = Arc::new(st);
    let p = StashPut::new(Arc::clone(&st), Arc::new(Steam), 1 << 20);
    put(&p, 101, 9, 500); // an old build's chunk
    st.close_segment_of(&GameDir::new("steam", "101"));
    put(&p, 101, 1, 100);
    put(&p, 101, 2, 200);
    st.close_segment_of(&GameDir::new("steam", "101"));
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    lib.refresh().unwrap();
    let grace = 7 * 86_400_000;
    let pruner = SteamPruner::new(s, lib, Arc::clone(&st), clock.clone(), grace);
    let fresh = pruner.find("100").unwrap();
    assert!(fresh.items.is_empty() && fresh.recent_bytes > 0, "used lately: kept for now ({fresh:?})");
    clock.0.store(1_000 + grace + 1, Ordering::SeqCst);
    let old = pruner.find("100").unwrap();
    assert_eq!(old.items.len(), 1, "only the old build's segment: {old:?}");
    assert!(matches!(&old.items[0], PruneItem::Segment { dir, .. } if dir == "101"));
    assert_eq!(old.recent_bytes, 0);
    assert!(view("100", &old).starts_with("{\"game\":\"100\",\"files\":1,\"compact\":0,\"bytes\":"));
    assert!(pruner.find("7").is_err(), "not in the library");
}

#[test]
fn a_dlc_with_a_depot_of_its_own_is_pruned_with_its_game_and_counts_toward_its_build() {
    use lanstash_steam_app::builds::SteamBuilds;
    use lanstash_steam_domain::apps::Target;
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let (st, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let st = Arc::new(st);
    let p = StashPut::new(Arc::clone(&st), Arc::new(Steam), 1 << 20);
    put(&p, 101, 1, 100);
    put(&p, 101, 2, 200);
    put(&p, 102, 3, 40);
    st.close_segment_of(&GameDir::new("steam", "101"));
    put(&p, 201, 6, 300); // an old build of the DLC's, alone in its segment
    st.close_segment_of(&GameDir::new("steam", "201"));
    let (s, _) = support::account::services_with(true, true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    lib.refresh().unwrap();
    let builds = SteamBuilds::new(s.clone(), Arc::clone(&lib), Arc::clone(&st), Target::default(), false);
    builds.refresh().unwrap();
    assert!(builds.current().is_empty(), "the DLC's current chunk is missing: not whole");
    put(&p, 201, 5, 20);
    st.close_segment_of(&GameDir::new("steam", "201"));
    builds.refresh().unwrap();
    assert_eq!(builds.current(), [100], "whole with the DLC's depot");
    let grace = 7 * 86_400_000;
    clock.0.store(1_000 + grace + 1, Ordering::SeqCst);
    let old = SteamPruner::new(s, lib, Arc::clone(&st), clock.clone(), grace).find("100").unwrap();
    assert_eq!(old.items.len(), 1, "{old:?}");
    assert!(matches!(&old.items[0], PruneItem::Segment { dir, .. } if dir == "201"), "{old:?}");
}

#[test]
fn the_automatic_prune_takes_an_app_stashed_whole_unless_it_is_being_prefilled() {
    use lanstash_app::delete_game::{DeleteState, GameDeleter};
    use lanstash_ports::events::NoEvents;
    use lanstash_steam_app::builds::SteamBuilds;
    use lanstash_steam_app::prune::prune_once;
    use lanstash_steam_domain::apps::Target;
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let (st, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let st = Arc::new(st);
    let p = StashPut::new(Arc::clone(&st), Arc::new(Steam), 1 << 20);
    put(&p, 101, 9, 500); // an old build's chunk, alone in its segment
    st.close_segment_of(&GameDir::new("steam", "101"));
    put(&p, 101, 1, 100);
    put(&p, 101, 2, 200);
    put(&p, 102, 3, 40);
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    lib.refresh().unwrap();
    let builds = SteamBuilds::new(s.clone(), Arc::clone(&lib), Arc::clone(&st), Target::default(), false);
    builds.refresh().unwrap();
    assert_eq!(builds.current(), [100], "stashed whole");
    let grace = 7 * 86_400_000;
    let pruner = SteamPruner::new(s, lib, Arc::clone(&st), clock.clone(), grace);
    let deleter = GameDeleter::new(Arc::clone(&st), Arc::new(NoEvents), clock.clone());
    assert_eq!(prune_once(&pruner, &builds, &deleter, &|_| false), None, "nothing past the grace period yet");
    clock.0.store(1_000 + grace + 1, Ordering::SeqCst);
    assert_eq!(prune_once(&pruner, &builds, &deleter, &|g| g == "100"), None, "being prefilled");
    let (game, bytes) = prune_once(&pruner, &builds, &deleter, &|_| false).unwrap();
    assert!(game == "100" && bytes > 0);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while deleter.status().is_some_and(|s| s.state == DeleteState::Running) && std::time::Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(builds.current(), [100], "the current build is untouched");
}

#[test]
fn a_segment_mostly_of_old_chunks_is_compacted_and_one_mostly_current_stays() {
    use lanstash_app::delete_game::{DeleteState, GameDeleter};
    use lanstash_domain::object::object_key;
    use lanstash_ports::events::NoEvents;
    use lanstash_store::index::Lookup;
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let fs = MemFs::default();
    let (st, _) = Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), clock.clone()).unwrap();
    let st = Arc::new(st);
    let p = StashPut::new(Arc::clone(&st), Arc::new(Steam), 1 << 20);
    put(&p, 101, 9, 300_000); // an old build's chunk next to a current one: mixed, mostly old
    put(&p, 101, 1, 100);
    st.close_segment_of(&GameDir::new("steam", "101"));
    put(&p, 102, 8, 1_000); // an old one next to a big current one: mixed, mostly current
    put(&p, 102, 3, 200_000);
    st.close_segment_of(&GameDir::new("steam", "102"));
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    lib.refresh().unwrap();
    let grace = 7 * 86_400_000;
    let pruner = SteamPruner::new(s, lib, Arc::clone(&st), clock.clone(), grace);
    let fresh = pruner.find("100").unwrap();
    assert!(fresh.items.is_empty() && fresh.recent_bytes >= 300_000, "used lately: {fresh:?}");
    clock.0.store(1_000 + grace + 1, Ordering::SeqCst);
    let old = pruner.find("100").unwrap();
    assert_eq!((old.items.len(), old.compact), (1, 1), "{old:?}");
    let PruneItem::Compact { dir, seq: 0, keep } = &old.items[0] else { panic!("{old:?}") };
    assert_eq!(dir, "101");
    let key =
        |depot: u32, sha: u8| object_key("steam", &format!("/depot/{depot}/chunk/{}", format!("{sha:02x}").repeat(20)));
    assert_eq!(keep, &[key(101, 1)]);
    assert!(view("100", &old).contains("\"files\":1,\"compact\":1,"));

    let deleter = GameDeleter::new(Arc::clone(&st), Arc::new(NoEvents), clock.clone());
    deleter.start_items("steam", "100", old.items).unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while deleter.status().is_some_and(|s| s.state == DeleteState::Running) && std::time::Instant::now() < until {
        std::thread::yield_now();
    }
    let done = deleter.status().unwrap();
    assert!(done.failed.is_empty() && done.items_done == 1 && done.bytes_kept == 100, "{done:?}");
    assert!(fs.get("steam/101/seg/00000.lcseg").is_none(), "the mixed segment went");
    assert!(matches!(st.lookup_quietly(&key(101, 1)), Lookup::Found(_)), "the current chunk was kept");
    assert!(matches!(st.lookup_quietly(&key(101, 9)), Lookup::Missing), "the old one went");
    assert!(matches!(st.lookup_quietly(&key(102, 8)), Lookup::Found(_)), "the mostly current segment stays");
    assert!(pruner.find("100").unwrap().items.is_empty(), "nothing more to do");
}
