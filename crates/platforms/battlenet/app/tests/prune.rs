//! Prune: what of a game's directory no current build needs, for any platform
//! or language, kept while used within the grace period.

mod support;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lanstash_app::test_support::FakeResponse;
use lanstash_battlenet_app::plans::{BlizzardPlans, PlanOptions};
use lanstash_battlenet_app::prune::Pruner;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState};
use support::*;

const DAY: u64 = 86_400_000;

/// A Pruner over the fixture, with plans of every platform and language.
fn pruner(w: &Bnet, grace_ms: u64) -> Pruner {
    let full = PlanOptions { region: "eu".into(), tags: Vec::new(), gap: 64, ..PlanOptions::default() };
    let plans =
        Arc::new(BlizzardPlans::new(w.cache.clone(), Arc::clone(&w.store), w.up.clone(), w.clock.clone(), full));
    Pruner::new(Arc::clone(&w.builds), plans, Arc::clone(&w.store), w.clock.clone(), grace_ms)
}

#[test]
fn only_what_no_current_build_needs_and_nobody_used_lately() {
    let w = bnet(false);
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while w.prefill.status().unwrap().state == PrefillState::Running && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    w.settle();
    // An archive of an old build, in part (an object file): no current build lists it.
    let old = at("data", "0d000000000000000000000000000077", "");
    w.up.route(CDN, &old, FakeResponse::ranged(&[7u8; 4096], &[]));
    w.get(&old, Some("bytes=0-999"));
    assert!(w.holds(&old, Some(lanstash_domain::extent::Span::new(0, 1000))));
    // Within the grace period it stays; the install's archives always do.
    let p = pruner(&w, 7 * DAY);
    let now = pruner(&w, 7 * DAY).find("osi").unwrap();
    assert_eq!((now.files.len(), now.bytes), (0, 0), "{now:?}");
    assert!(now.recent_bytes >= 1000, "{now:?}");
    // A week later nobody used it: prunable, alone.
    w.clock.0.fetch_add(8 * DAY, Ordering::SeqCst);
    let later = p.find("osi").unwrap();
    assert_eq!(later.files.len(), 1, "{later:?}");
    assert!(later.bytes >= 1000 && later.recent_bytes == 0, "{later:?}");
    // A game whose build cannot be looked up has nothing prunable.
    assert!(p.find("nothing").is_err());
}

#[test]
fn the_automatic_pass_prunes_a_game_whose_live_build_is_stashed() {
    use lanstash_app::delete_game::{DeleteState, GameDeleter};
    use lanstash_battlenet_app::prune::prune_once;
    use lanstash_ports::events::NoEvents;
    let w = bnet(false);
    let p = pruner(&w, 7 * DAY);
    let deleter = GameDeleter::new(Arc::clone(&w.store), Arc::new(NoEvents), w.clock.clone());
    // Not stashed whole yet: nothing to do, whatever is old.
    let old = at("data", "0d000000000000000000000000000077", "");
    w.up.route(CDN, &old, FakeResponse::ranged(&[7u8; 4096], &[]));
    w.get(&old, Some("bytes=0-999"));
    w.clock.0.fetch_add(8 * DAY, Ordering::SeqCst);
    assert_eq!(prune_once(&p, &w.builds, &deleter, &|_| false), None, "the live build is not stashed whole");
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while w.prefill.status().unwrap().state == PrefillState::Running && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    w.settle();
    w.clock.0.fetch_add(8 * DAY, Ordering::SeqCst);
    assert_eq!(prune_once(&p, &w.builds, &deleter, &|_| true), None, "not while it is being prefilled");
    let (game, bytes) = prune_once(&p, &w.builds, &deleter, &|_| false).expect("pruned");
    assert!(game == "osi" && bytes >= 1000);
    let until = Instant::now() + Duration::from_secs(5);
    while deleter.status().unwrap().state == DeleteState::Running && Instant::now() < until {
        std::thread::yield_now();
    }
    assert!(deleter.status().unwrap().partial);
    assert!(!w.holds(&old, Some(lanstash_domain::extent::Span::new(0, 1000))), "the old archive went");
    assert!(w.planned().current(), "the live build stays whole");
}
