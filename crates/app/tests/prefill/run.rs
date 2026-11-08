//! What a job does with each request: skip, fetch, count as missing, or take
//! out of lancache's cache when it passes the platform's check.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_app::test_support::FakeResponse;
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState};
use lanstash_store::index::Lookup;
use lanstash_store::test_support::nginx::MemNginx;

use super::{CDN, Fixed, World, job, world};

fn holds(w: &World, target: &str, span: Span) -> bool {
    match w.store.index().lookup(&object_key("blizzard", target)) {
        Lookup::Found(o) => o.covers(span),
        _ => false,
    }
}

#[test]
fn requests_go_through_the_cache_and_what_is_held_is_skipped() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/a", FakeResponse::ranged(b"0123456789", &[]));
    w.up.route(CDN, "/tpr/osi/b", FakeResponse::ranged(&[7u8; 100], &[]));
    let jobs = vec![job("/tpr/osi/a", None), job("/tpr/osi/b", Some((10, 30))), job("/tpr/osi/gone", None)];
    let p = w.prefiller(Arc::new(Fixed::new(jobs.clone())));
    w.up.route(CDN, "/tpr/osi/gone", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_total, s.bytes_total), (PrefillState::Done, 3, 30), "{s:?}");
    assert_eq!((s.files_done, s.files_skipped, s.files_missing), (3, 0, 1), "{s:?}");
    assert!(holds(&w, "/tpr/osi/a", Span::new(0, 10)) && holds(&w, "/tpr/osi/b", Span::new(10, 30)));
    // Again: everything there is, is held.
    let calls = w.up.calls();
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.files_skipped, s.files_missing), (2, 1), "{s:?}");
    assert_eq!(w.up.calls(), calls + 1, "only the missing file is asked again");
}

/// A pause that counts what it was asked to wait and, the first time, mends the CDN.
struct Mending {
    waited: std::sync::Mutex<Vec<u64>>,
    mend: Box<dyn Fn() + Send + Sync>,
}

impl lanstash_ports::clock::Pause for Mending {
    fn pause_ms(&self, ms: u64) {
        let mut w = self.waited.lock().unwrap();
        if w.is_empty() {
            (self.mend)();
        }
        w.push(ms);
    }
}

#[test]
fn a_request_the_cdn_fails_once_is_tried_again_after_a_pause() {
    let w = world();
    // A 502 for the one chunk, as a CDN answers now and then.
    w.up.route(CDN, "/tpr/osi/a", FakeResponse { status: 502, ..FakeResponse::ok(b"bad gateway", &[]) });
    let up = w.up.clone();
    let pause = Arc::new(Mending {
        waited: std::sync::Mutex::new(Vec::new()),
        mend: Box::new(move || up.route(CDN, "/tpr/osi/a", FakeResponse::ranged(b"0123456789", &[]))),
    });
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/tpr/osi/a", None)]))).with_pause(pause.clone());
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_done, s.error.as_deref()), (PrefillState::Done, 1, None), "{s:?}");
    assert!(holds(&w, "/tpr/osi/a", Span::new(0, 10)));
    assert_eq!(*pause.waited.lock().unwrap(), [1_000], "one pause, of the first try's length");
    assert_eq!(s.bytes, 10, "the error page of the failed try does not count");
}

#[test]
fn the_bytes_of_a_try_that_broke_off_count_once() {
    let w = world();
    // More breaks than the cache's own retries: the first try of the request fails.
    let left = Arc::new(std::sync::atomic::AtomicUsize::new(4));
    let broken = FakeResponse { fail_after: Some(5), fail_times: Some(left), ..FakeResponse::ok(b"0123456789", &[]) };
    w.up.route(CDN, "/tpr/osi/a", broken);
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/tpr/osi/a", None)])));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.bytes), (PrefillState::Done, 10), "{s:?}");
}

#[test]
fn a_failed_request_fails_the_job() {
    let w = world();
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/tpr/osi/unrouted", None)])));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!(s.state, PrefillState::Failed);
    assert!(s.error.as_deref().is_some_and(|e| e.ends_with("(tried 3 times)")), "{s:?}");
    let mut planner = Fixed::new(Vec::new());
    planner.fail = Some("osi: not a product".into());
    let p = w.prefiller(Arc::new(planner));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = w.wait(&p);
    assert_eq!(
        (s.state, s.error.as_deref(), s.version.as_deref()),
        (PrefillState::Failed, Some("osi: not a product"), Some("1.0"))
    );
}

#[test]
fn checked_pieces_come_from_lancache_the_rest_from_the_cdn() {
    let w = world();
    let body: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    w.up.route(CDN, "/tpr/osi/big", FakeResponse::ranged(&body, &[]));
    w.up.route(CDN, "/tpr/osi/small", FakeResponse::ranged(b"small", &[]));
    let nginx = Arc::new(MemNginx::default());
    nginx.whole("blizzard/tpr/osi/big", &body, 1024);
    let planner = Arc::new(Fixed::new(vec![job("/tpr/osi/big", Some((100, 2100))), job("/tpr/osi/small", None)]));
    let p = w.with_lancache(planner.clone(), &nginx);
    assert!(p.lancache());
    p.start("osi", false, PrefillFrom::Lancache).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_local, s.bytes_local, s.files_rejected), (PrefillState::Done, 1, 2000, 0), "{s:?}");
    assert_eq!(w.up.calls(), 1, "only the file lancache lacks came from the CDN");
    assert!(holds(&w, "/tpr/osi/big", Span::new(100, 2100)));
    // A piece that fails its check is fetched instead.
    let w = world();
    w.up.route(CDN, "/tpr/osi/big", FakeResponse::ranged(&body, &[]));
    w.up.route(CDN, "/tpr/osi/small", FakeResponse::ranged(b"small", &[]));
    planner.accept.store(false, Ordering::SeqCst);
    let p = w.with_lancache(planner, &nginx);
    p.start("osi", false, PrefillFrom::Lancache).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_local, s.files_rejected), (PrefillState::Done, 0, 1), "{s:?}");
    assert_eq!(w.up.calls(), 2);
    assert!(holds(&w, "/tpr/osi/big", Span::new(100, 2100)));
}

#[test]
fn of_a_range_lancache_holds_only_some_of_it_gives_the_parts_it_holds() {
    let w = world();
    let body: Vec<u8> = (0..3072u32).map(|i| (i % 251) as u8).collect();
    w.up.route(CDN, "/tpr/osi/archive", FakeResponse::ranged(&body, &[]));
    // Three files of 1 KiB each in one range; lancache lacks the slice of the middle one.
    let nginx = Arc::new(MemNginx::default());
    nginx.whole("blizzard/tpr/osi/archive", &body, 1024);
    nginx.remove(&lanstash_domain::nginx_cache::slice_key("blizzard/tpr/osi/archive", 1024, 1024));
    let mut planner = Fixed::new(vec![job("/tpr/osi/archive", Some((0, 3072)))]);
    planner.parts = vec![vec![(0, 1024), (1024, 2048), (2048, 3072)]];
    let p = w.with_lancache(Arc::new(planner), &nginx);
    p.start("osi", false, PrefillFrom::Lancache).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_done, s.files_local, s.files_rejected), (PrefillState::Done, 1, 0, 0), "{s:?}");
    assert_eq!(s.bytes_local, 2048, "the first and last file came from lancache");
    assert_eq!(s.bytes, 3072, "and only the middle one counts as fetched: {s:?}");
    assert!(holds(&w, "/tpr/osi/archive", Span::new(0, 3072)), "the whole range is stashed");
    let fetched: Vec<Option<String>> =
        w.up.seen
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.headers.iter().find(|h| h.0.eq_ignore_ascii_case("range")).map(|h| h.1.clone()))
            .collect();
    assert_eq!(fetched, vec![Some("bytes=1024-2047".to_owned())], "the CDN was asked for the middle file alone");
}

#[test]
fn lancache_only_takes_what_lancache_holds_and_asks_the_cdn_for_nothing() {
    let w = world();
    let body: Vec<u8> = (0..3072u32).map(|i| (i % 251) as u8).collect();
    let nginx = Arc::new(MemNginx::default());
    nginx.whole("blizzard/tpr/osi/big", &body, 1024);
    nginx.whole("blizzard/tpr/osi/archive", &body, 1024);
    nginx.remove(&lanstash_domain::nginx_cache::slice_key("blizzard/tpr/osi/archive", 1024, 1024));
    let mut planner = Fixed::new(vec![
        job("/tpr/osi/big", Some((0, 2048))),
        job("/tpr/osi/archive", Some((0, 3072))),
        job("/tpr/osi/small", None),
    ]);
    planner.parts = vec![Vec::new(), vec![(0, 1024), (1024, 2048), (2048, 3072)]];
    let p = w.with_lancache(Arc::new(planner), &nginx);
    p.start("osi", false, PrefillFrom::LancacheOnly).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_done, s.files_local, s.files_missing), (PrefillState::Done, 3, 1, 2), "{s:?}");
    assert_eq!(s.bytes_local, 2048 + 2048, "the whole range, and two files of the partial one");
    assert_eq!(w.up.calls(), 0, "nothing from the CDN");
    assert!(holds(&w, "/tpr/osi/big", Span::new(0, 2048)));
    assert!(
        holds(&w, "/tpr/osi/archive", Span::new(2048, 3072)) && !holds(&w, "/tpr/osi/archive", Span::new(1024, 2048))
    );
    assert_eq!(PrefillFrom::LancacheOnly.as_str(), "lancache-only");
}
