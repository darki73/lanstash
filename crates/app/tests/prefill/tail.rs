//! A whole file lancache holds all but the end of: the start from lancache,
//! the end from the CDN, checked whole, stored as one.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_app::test_support::FakeResponse;
use lanstash_domain::object::object_key;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState};
use lanstash_store::index::Lookup;
use lanstash_store::test_support::nginx::MemNginx;

use super::{CDN, Fixed, World, job, world};

fn stored(w: &World, target: &str) -> Option<u64> {
    match w.store.index().lookup(&object_key("blizzard", target)) {
        Lookup::Found(o) if o.covers(lanstash_domain::extent::Span::new(0, o.total)) => Some(o.total),
        _ => None,
    }
}

fn ranges(w: &World) -> Vec<Option<String>> {
    w.up.seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.headers.iter().find(|h| h.0.eq_ignore_ascii_case("range")).map(|h| h.1.clone()))
        .collect()
}

#[test]
fn the_start_comes_from_lancache_and_only_the_missing_end_from_the_cdn() {
    let w = world();
    let body: Vec<u8> = (0..1500u32).map(|i| (i % 251) as u8).collect();
    w.up.route(CDN, "/tpr/osi/chunk", FakeResponse::ranged(&body, &[]));
    let nginx = Arc::new(MemNginx::default());
    nginx.slice("blizzard/tpr/osi/chunk", &body, 0, 1024); // slice 1 never fetched
    let p = w.with_lancache(Arc::new(Fixed::new(vec![job("/tpr/osi/chunk", None)])), &nginx).with_tails(w.up.clone());
    p.start("osi", false, PrefillFrom::LancacheOnly).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.state, s.files_local, s.files_missing), (PrefillState::Done, 1, 0), "{s:?}");
    assert_eq!((s.bytes, s.bytes_local), (1500, 1024), "{s:?}");
    assert_eq!(ranges(&w), vec![Some("bytes=1024-1499".to_owned())], "the CDN gave the end alone");
    assert_eq!(stored(&w, "/tpr/osi/chunk"), Some(1500), "stored whole");
}

#[test]
fn a_large_end_a_failed_check_or_no_tails_leave_it_out() {
    let body: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    let nginx = Arc::new(MemNginx::default());
    nginx.slice("blizzard/tpr/osi/big", &body, 0, 1024); // 1976 bytes missing: more than lancache holds
    nginx.slice("blizzard/tpr/osi/odd", &body[..1500], 0, 1024);
    // The end is more than the start.
    let w = world();
    let p = w.with_lancache(Arc::new(Fixed::new(vec![job("/tpr/osi/big", None)])), &nginx).with_tails(w.up.clone());
    p.start("osi", false, PrefillFrom::LancacheOnly).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.files_local, s.files_missing, w.up.calls()), (0, 1, 0), "{s:?}");
    // The whole file fails its check: nothing stored.
    let w = world();
    w.up.route(CDN, "/tpr/osi/odd", FakeResponse::ranged(&body[..1500], &[]));
    let planner = Fixed::new(vec![job("/tpr/osi/odd", None)]);
    planner.accept.store(false, Ordering::SeqCst);
    let p = w.with_lancache(Arc::new(planner), &nginx).with_tails(w.up.clone());
    p.start("osi", false, PrefillFrom::LancacheOnly).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.files_local, s.files_rejected, s.files_missing), (0, 1, 1), "{s:?}");
    assert_eq!(stored(&w, "/tpr/osi/odd"), None);
    // Without tails, lancache-only leaves it out and asks the CDN nothing.
    let w = world();
    let p = w.with_lancache(Arc::new(Fixed::new(vec![job("/tpr/osi/odd", None)])), &nginx);
    p.start("osi", false, PrefillFrom::LancacheOnly).unwrap();
    let s = w.wait(&p);
    assert_eq!((s.files_local, s.files_missing, w.up.calls()), (0, 1, 0), "{s:?}");
}
