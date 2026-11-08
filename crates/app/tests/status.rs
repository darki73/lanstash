//! Status snapshots from a running cache.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_app::fetch::{FetchOptions, Fetcher};
use lanstash_app::passthrough::Passthrough;
use lanstash_app::serve::{Cache, ConnState, RequestHandler, ServeOptions};
use lanstash_app::status::Status;
use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream};
use lanstash_domain::http::RequestHead;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::Clock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::names::GameNames;
use lanstash_store::layout::GameDir;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

struct Names;
impl GameNames for Names {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (service == "steam" && game == "7").then(|| "Seven".to_owned())
    }
}

struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// A cache over an empty stash, its status (named by `names`, walked at least every
/// `max_age_ms`, a change walked 5 s after the last walk at the soonest) and its clock.
fn rig(names: Arc<dyn GameNames>, max_age_ms: u64) -> (Arc<TestClock>, Arc<Cache>, Status) {
    let clock = Arc::new(TestClock(AtomicU64::new(1_000)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let up = Arc::new(FakeUpstream::default());
    up.route(
        "cache1.steamcontent.com",
        "/depot/7/chunk/0123456789abcdef0123456789abcdef01234567",
        FakeResponse::ok(b"chunk", &[]),
    );
    let mut hosts = HostRules::default();
    hosts.add_host("steam", "steamcontent.com");
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), FetchOptions::default()));
    let pass = Arc::new(Passthrough::new(up, 1 << 20, 4096));
    let cache = Arc::new(
        Cache::new(
            Arc::clone(&store),
            hosts,
            &support::profiles::profiles(),
            Arc::new(NoEvents),
            ServeOptions::default(),
        )
        .with_upstream(fetcher, pass),
    );
    let status =
        Status::new(Arc::clone(&cache), Arc::clone(&store), clock.clone(), names, max_age_ms).with_usage_min_age(5_000);
    (clock, cache, status)
}

/// Asks the cache for one Steam chunk and waits for its download to be stored.
fn get_chunk(cache: &Cache) {
    let req = RequestHead {
        method: "GET".into(),
        target: "/depot/7/chunk/0123456789abcdef0123456789abcdef01234567".into(),
        minor_version: 1,
        headers: vec![("host".into(), "cache1.steamcontent.com".into())],
    };
    cache.handle(&req, &mut ConnState::default(), &mut Capture::default()).unwrap();
    while cache.fetcher().unwrap().in_flight() > 0 {
        std::thread::yield_now();
    }
}

#[test]
fn snapshot_shows_counters_and_cached_usage() {
    let (clock, cache, status) = rig(Arc::new(Names), 30_000);
    let before = status.snapshot();
    assert!(before.usage.is_empty() && before.downloads.is_empty());
    assert_eq!(
        before.counters.iter().map(|g| g.name).collect::<Vec<_>>(),
        vec!["requests", "downloads", "pass-through", "storage"]
    );

    get_chunk(&cache);

    clock.0.store(2_000, Ordering::Relaxed);
    let s = status.snapshot();
    let requests = &s.counters[0].values;
    assert!(requests.contains(&("requests", 1)) && requests.contains(&("misses", 1)), "{requests:?}");
    assert!(s.usage.is_empty(), "a change is walked 5 s after the last walk at the soonest");
    clock.0.store(6_000, Ordering::Relaxed);
    let s = status.snapshot();
    assert_eq!(s.usage.len(), 1, "the stash changed: walked again");
    let row = &s.usage[0];
    assert_eq!((&row.game, row.name.as_deref()), (&GameDir::new("steam", "7"), Some("Seven")));
    assert_eq!((row.usage.objects, row.usage.bytes, s.usage_ms), (1, 5, 6_000));
    clock.0.store(20_000, Ordering::Relaxed);
    assert_eq!(status.snapshot().usage_ms, 6_000, "nothing changed: the figures stand");
    clock.0.store(36_000, Ordering::Relaxed);
    assert_eq!(status.snapshot().usage_ms, 36_000, "walked again every 30 s (when games were last used)");
}

/// Names a source learns late (Steam's, once its library is read).
#[derive(Default)]
struct Late(std::sync::atomic::AtomicBool);
impl GameNames for Late {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (self.0.load(Ordering::Relaxed) && service == "steam" && game == "7").then(|| "Seven".to_owned())
    }
}

#[test]
fn names_learnt_late_come_in_without_a_walk_and_counters_need_none() {
    use lanstash_app::status::NAMES_MAX_AGE_MS;
    let names = Arc::new(Late::default());
    let (clock, cache, status) = rig(names.clone(), 600_000);
    get_chunk(&cache);
    clock.0.store(10_000, Ordering::Relaxed);
    let s = status.snapshot();
    assert_eq!((s.usage_ms, s.usage[0].name.as_deref()), (10_000, None));
    names.0.store(true, Ordering::Relaxed);
    clock.0.store(10_000 + NAMES_MAX_AGE_MS, Ordering::Relaxed);
    let s = status.snapshot();
    assert_eq!(s.usage[0].name.as_deref(), Some("Seven"), "named again");
    assert_eq!(s.usage_ms, 10_000, "without walking the index again");
    let names_of = |g: &[lanstash_app::status::CounterGroup]| g.iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(names_of(&status.counters()), names_of(&s.counters));
}
