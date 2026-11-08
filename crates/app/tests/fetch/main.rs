//! Tests for `lanstash_app::fetch`: planning downloads against the index and
//! the in-flight table, without a request handler in front.

mod counters;
#[path = "../support/mod.rs"]
mod support;
mod transfer;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lanstash_app::fetch::{Cover, FetchOptions, Fetcher, Known, ObjectInfo};
use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_domain::service::Profile;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

const HOST: &str = "cdn.example";

struct Rig {
    up: Arc<FakeUpstream>,
    store: Arc<Store>,
    fetcher: Arc<Fetcher>,
}

fn rig() -> Rig {
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 1 << 20, alignment: 4096 }, read_only: false };
    let (store, _) = Store::open(Roots::single(Arc::new(MemFs::default())), opts, Arc::new(FixedClock(1))).unwrap();
    let store = Arc::new(store);
    let up = Arc::new(FakeUpstream::default());
    let fetch = FetchOptions { piece: 100, ahead: 2, window: 64, object_file_min: 4096, ..FetchOptions::default() };
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), fetch));
    Rig { up, store, fetcher }
}

/// An object of `profile` at `target` on [`HOST`].
fn object(profile: Arc<dyn Profile>, target: &str) -> Arc<ObjectInfo> {
    Arc::new(ObjectInfo {
        service: profile.service().to_owned(),
        game: "g".into(),
        key: format!("{}{target}", profile.service()),
        path: target.into(),
        host: HOST.into(),
        target: target.into(),
        user_agent: None,
        profile,
    })
}

fn body(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251) as u8).collect()
}

impl Rig {
    /// Waits until no download is running (stores are committed by then).
    fn settle(&self) {
        let until = Instant::now() + Duration::from_secs(5);
        while self.fetcher.in_flight() > 0 {
            assert!(Instant::now() < until, "downloads did not finish");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

#[test]
fn a_probe_learns_the_length_and_others_join_it() {
    let r = rig();
    r.up.route(HOST, "/tpr/g/a", FakeResponse::ranged(&body(250), &[]));
    let obj = object(Arc::new(support::profiles::Ranged), "/tpr/g/a");
    r.up.hold();
    let probe = {
        let (f, o) = (Arc::clone(&r.fetcher), Arc::clone(&obj));
        std::thread::spawn(move || f.learn(&o, 0, None))
    };
    let until = Instant::now() + Duration::from_secs(5);
    while r.fetcher.downloads().is_empty() {
        assert!(Instant::now() < until, "the probe never started");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(r.fetcher.in_flight(), 1);
    r.up.release();
    let Known::Fetching { total, first, .. } = probe.join().unwrap() else { panic!("not fetching") };
    assert_eq!(total, Some(250));
    first.wait_end().unwrap();
    r.settle();
    assert_eq!(r.up.calls(), 1, "the probe fetched one piece");

    // The rest is planned from the stored first piece, in pieces.
    let Cover::Download(d) = r.fetcher.cover(&obj, 100, 250, 250) else { panic!("nothing started") };
    d.wait_end().unwrap();
    r.settle();
    assert!(matches!(r.fetcher.cover(&obj, 0, 250, 250), Cover::Stored));
    let Lookup::Found(o) = r.store.lookup(&obj.key) else { panic!() };
    assert!(o.covers(lanstash_domain::extent::Span::new(0, 250)));
    assert_eq!(r.fetcher.counters.started.load(Ordering::Relaxed), 3, "probe, then two pieces ahead");
}

#[test]
fn a_piece_answered_with_an_error_fails_and_frees_its_span() {
    let r = rig();
    r.up.route(HOST, "/tpr/g/e", FakeResponse::ranged(&body(300), &[]));
    let obj = object(Arc::new(support::profiles::Ranged), "/tpr/g/e");
    let Known::Fetching { first, .. } = r.fetcher.learn(&obj, 0, None) else { panic!("not fetching") };
    first.wait_end().unwrap();
    r.settle();

    r.up.route(HOST, "/tpr/g/e", FakeResponse { status: 503, ..FakeResponse::ok(b"busy", &[]) });
    let Cover::Download(d) = r.fetcher.cover(&obj, 100, 300, 300) else { panic!("nothing started") };
    assert!(d.wait_end().is_err(), "an error page is not the object's bytes");
    r.settle();

    // The failed piece holds nothing: the next cover starts it again.
    r.up.route(HOST, "/tpr/g/e", FakeResponse::ranged(&body(300), &[]));
    let Cover::Download(d) = r.fetcher.cover(&obj, 100, 300, 300) else { panic!("the span stayed taken") };
    d.wait_end().unwrap();
    r.settle();
    let Lookup::Found(o) = r.store.lookup(&obj.key) else { panic!() };
    assert!(o.covers(lanstash_domain::extent::Span::new(0, 300)));
}

#[test]
fn stored_and_unreachable_objects_are_known_at_once() {
    let r = rig();
    r.up.route(HOST, "/depot/1/chunk/x", FakeResponse::ok(&body(10), &[]));
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/x");
    let Known::Fetching { first, .. } = r.fetcher.learn(&obj, 0, None) else { panic!() };
    first.wait_end().unwrap();
    r.settle();
    assert!(matches!(r.fetcher.learn(&obj, 0, None), Known::Stored(_)));
    assert_eq!(r.up.calls(), 1);

    let missing = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/none");
    assert!(matches!(r.fetcher.learn(&missing, 0, None), Known::Failed(_)));
    r.settle();
    assert!(r.fetcher.downloads().is_empty());
}
