//! Tests for `lanstash_app::kept_builds`: the builds are worked out when asked to
//! and kept, a request costs the platform nothing once they are, and a change is noted.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::kept_builds::{KeepOptions, KeptBuilds};
use lanstash_ports::clock::Clock;
use lanstash_ports::platform::{BuildStatus, Platform, View};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

/// A platform whose builds view is whatever `view` holds; counts how often it is asked.
#[derive(Debug, Default)]
struct Counted {
    view: Mutex<String>,
    asked: AtomicU64,
}

impl Platform for Counted {
    fn id(&self) -> &str {
        "steam"
    }
    fn builds(&self) -> Option<View> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        Some(Ok(self.view.lock().unwrap().clone()))
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        Some(Vec::new())
    }
}

struct Ticking(AtomicU64);

impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

fn kept(inner: &Arc<Counted>, clock: &Arc<Ticking>) -> KeptBuilds {
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    KeptBuilds::new(inner.clone(), Arc::new(store), clock.clone(), KeepOptions::default())
}

#[test]
fn once_worked_out_the_builds_are_answered_without_asking_the_platform() {
    let inner = Arc::new(Counted::default());
    *inner.view.lock().unwrap() = "a".into();
    let clock = Arc::new(Ticking(AtomicU64::new(10)));
    let k = kept(&inner, &clock);
    k.refresh();
    assert_eq!(inner.asked.load(Ordering::Relaxed), 1);
    for _ in 0..5 {
        assert_eq!(k.builds(), Some(Ok("a".into())));
    }
    assert_eq!(k.build_status(), Some(Vec::new()));
    assert_eq!(inner.asked.load(Ordering::Relaxed), 1, "the kept builds answer");
    assert_eq!(k.id(), "steam");
}

#[test]
fn asked_before_the_first_run_a_request_works_them_out_itself() {
    let inner = Arc::new(Counted::default());
    *inner.view.lock().unwrap() = "a".into();
    let k = kept(&inner, &Arc::new(Ticking(AtomicU64::new(1))));
    assert_eq!(k.builds(), Some(Ok("a".into())));
    assert_eq!(k.builds(), Some(Ok("a".into())));
    assert_eq!(inner.asked.load(Ordering::Relaxed), 1);
}

#[test]
fn notes_when_the_builds_changed_and_only_then() {
    let inner = Arc::new(Counted::default());
    *inner.view.lock().unwrap() = "a".into();
    let clock = Arc::new(Ticking(AtomicU64::new(10)));
    let k = kept(&inner, &clock);
    assert_eq!(k.builds_changed_ms(), Some(0), "not worked out yet");
    k.refresh();
    assert_eq!(k.builds_changed_ms(), Some(10));
    clock.0.store(20, Ordering::Relaxed);
    k.refresh();
    assert_eq!(k.builds_changed_ms(), Some(10), "the same builds: no change");
    *inner.view.lock().unwrap() = "b".into();
    clock.0.store(30, Ordering::Relaxed);
    k.refresh();
    assert_eq!(k.builds_changed_ms(), Some(30));
    assert_eq!(k.builds(), Some(Ok("b".into())));
}

#[test]
fn each_run_is_noted_in_the_history() {
    use lanstash_app::build_history::BuildHistory;
    use lanstash_app::test_support::MemoryLogs;
    #[derive(Debug)]
    struct One(Mutex<String>);
    impl Platform for One {
        fn id(&self) -> &str {
            "steam"
        }
        fn build_status(&self) -> Option<Vec<BuildStatus>> {
            let v = self.0.lock().unwrap().clone();
            Some(vec![BuildStatus { game: "7".into(), version: v, known: true, current: false }])
        }
    }
    let clock = Arc::new(Ticking(AtomicU64::new(10)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let inner = Arc::new(One(Mutex::new("1".into())));
    let history = Arc::new(BuildHistory::new(Arc::new(MemoryLogs::default())));
    let k = KeptBuilds::new(inner.clone(), Arc::new(store), clock.clone(), KeepOptions::default())
        .with_history(Arc::clone(&history));
    k.refresh();
    *inner.0.lock().unwrap() = "2".into();
    clock.0.store(20, Ordering::Relaxed);
    k.refresh();
    let events = history.events(None, &[], 10);
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].at_ms, events[0].version.as_str(), events[0].from.as_deref()), (20, "2", Some("1")));
}

#[test]
fn the_platforms_own_builds_mark_shows_through() {
    #[derive(Debug)]
    struct Marked(AtomicU64);
    impl Platform for Marked {
        fn id(&self) -> &str {
            "epicgames"
        }
        fn builds_mark(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
    }
    let clock = Arc::new(Ticking(AtomicU64::new(1)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let inner = Arc::new(Marked(AtomicU64::new(3)));
    let k = KeptBuilds::new(inner.clone(), Arc::new(store), clock, KeepOptions::default());
    assert_eq!(k.builds_mark(), 3);
    inner.0.store(4, Ordering::Relaxed);
    assert_eq!(k.builds_mark(), 4, "what the keeper's thread waits on, with the stash's changes");
}

/// A platform that works its builds and status out together, counting how often; one of its
/// builds is never known (a product nobody installs).
#[derive(Debug, Default)]
struct Together {
    runs: AtomicU64,
}

impl Platform for Together {
    fn id(&self) -> &str {
        "blizzard"
    }
    fn builds(&self) -> Option<View> {
        panic!("asked alone: the keeper asks for both together")
    }
    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        panic!("asked alone: the keeper asks for both together")
    }
    fn builds_and_status(&self) -> lanstash_ports::platform::BuildsAndStatus {
        self.runs.fetch_add(1, Ordering::Relaxed);
        let never = BuildStatus { game: "classic".into(), version: "1".into(), known: false, current: false };
        (Some(Ok("v".into())), Some(vec![never]))
    }
}

#[test]
fn the_builds_and_status_are_worked_out_together() {
    let inner = Arc::new(Together::default());
    let clock = Arc::new(Ticking(AtomicU64::new(1)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let k = KeptBuilds::new(inner.clone(), Arc::new(store), clock, KeepOptions::default());
    k.refresh();
    assert_eq!((k.builds(), k.build_status().map(|s| s.len())), (Some(Ok("v".into())), Some(1)));
    assert_eq!(k.builds_and_status().0, Some(Ok("v".into())));
    assert_eq!(inner.runs.load(Ordering::Relaxed), 1);
}

/// A pause that moves the clock on instead of waiting; past `until_ms` it ends the thread.
struct Simulated {
    clock: Arc<Ticking>,
    until_ms: u64,
}

impl lanstash_ports::clock::Pause for Simulated {
    fn pause_ms(&self, ms: u64) {
        let now = self.clock.0.fetch_add(ms, Ordering::Relaxed) + ms;
        if now > self.until_ms {
            panic!("simulated time is up");
        }
    }
}

#[test]
fn a_build_never_known_does_not_cost_a_run_every_minute() {
    let inner = Arc::new(Together::default());
    let clock = Arc::new(Ticking(AtomicU64::new(0)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let opts = KeepOptions::default();
    let k = Arc::new(KeptBuilds::new(inner.clone(), Arc::new(store), clock.clone(), opts));
    // 30 minutes of nothing changing.
    let pause = Arc::new(Simulated { clock: Arc::clone(&clock), until_ms: 30 * 60_000 });
    k.keep_fresh(pause).unwrap();
    for _ in 0..2000 {
        if Arc::strong_count(&k) == 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(Arc::strong_count(&k), 1, "the simulated half hour ended the thread");
    // Runs at 0, then after 1, 2, 4 and 5 minutes, then every 5 (rests included): 9, not 30.
    let runs = inner.runs.load(Ordering::Relaxed);
    assert!((6..=10).contains(&runs), "{runs} runs in 30 minutes");
}

#[test]
fn a_platform_without_builds_has_no_mark_and_its_thread_ends() {
    #[derive(Debug)]
    struct Bare;
    impl Platform for Bare {
        fn id(&self) -> &str {
            "riot"
        }
    }
    let clock = Arc::new(Ticking(AtomicU64::new(1)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let k = Arc::new(KeptBuilds::new(Arc::new(Bare), Arc::new(store), clock, KeepOptions::default()));
    assert!(k.keep_fresh(Arc::new(lanstash_ports::clock::NoPause)).is_ok());
    assert_eq!(k.builds(), None);
    assert_eq!(k.builds_changed_ms(), None);
    assert_eq!(k.platform(), "riot");
    // With no pause at all, a thread that kept going would spin; one that ended leaves one owner.
    // Up to two seconds: the thread ends at once, but a loaded machine may run it late.
    for _ in 0..2000 {
        if Arc::strong_count(&k) == 1 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the thread of a platform without builds kept going");
}
