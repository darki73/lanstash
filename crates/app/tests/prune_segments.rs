//! Tests for `lanstash_app::prune_segments`: nothing outside the game's
//! directories is looked at, the automatic step does nothing without a game to
//! prune, and the view's shape. (The segment rules themselves run in the
//! platforms' prune tests, on stores with old and current builds.)

mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use lanstash_app::delete_game::GameDeleter;
use lanstash_app::prune_segments::{Prunable, prunable, prune_once, view};
use lanstash_app::stash_put::StashPut;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

fn store() -> Arc<Store> {
    let (s, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    Arc::new(s)
}

#[test]
fn only_the_games_directories_are_looked_at() {
    let s = store();
    let put = StashPut::new(Arc::clone(&s), Arc::new(support::profiles::Ranged), 100);
    put.put("/tpr/hs/data/aa/bb/keep", 0, b"hearthstone", 11, b"", true).unwrap();
    s.close_segments();
    let none = BTreeSet::new();
    let p = prunable(&s, "blizzard", &none, &[], 1_000_000, 0);
    assert_eq!(p, Prunable { items: Vec::new(), bytes: 0, recent_bytes: 0, compact: 0 }, "no directory named");
    let other = BTreeSet::from(["sc1live".to_string()]);
    assert!(prunable(&s, "blizzard", &other, &[], 1_000_000, 0).items.is_empty(), "another game's directory");
    assert!(prunable(&s, "steam", &BTreeSet::from(["hs".to_string()]), &[], 1_000_000, 0).items.is_empty());
}

#[test]
fn the_automatic_step_needs_a_game_with_something_to_prune() {
    let s = store();
    let d = GameDeleter::new(Arc::clone(&s), Arc::new(NoEvents), Arc::new(FixedClock(1)));
    let nothing = |_: &str| Ok(Prunable { items: Vec::new(), bytes: 0, recent_bytes: 0, compact: 0 });
    assert_eq!(prune_once("steam", &[], &nothing, &d, &|_| false), None, "no current game");
    assert_eq!(prune_once("steam", &["7".into()], &nothing, &d, &|_| false), None, "nothing to prune");
    let failing = |_: &str| Err("no manifest".to_string());
    assert_eq!(prune_once("steam", &["7".into()], &failing, &d, &|_| false), None);
    assert!(d.status().is_none(), "no deletion started");
}

#[test]
fn the_view_says_how_much_goes() {
    let p = Prunable { items: Vec::new(), bytes: 300, recent_bytes: 20, compact: 1 };
    assert_eq!(view("7", &p), "{\"game\":\"7\",\"files\":0,\"compact\":1,\"bytes\":300,\"recent_bytes\":20}\n");
}
