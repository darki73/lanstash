//! Tests for `lanstash_app::fetch::transfer`: one download against the origin.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_app::fetch::{FetchOptions, Fetcher, Known, VIA_HEADER};
use lanstash_app::test_support::FakeResponse;
use lanstash_domain::checksum::crc::crc32;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

use crate::{HOST, body, object, rig, support};

#[test]
fn redirects_are_followed_and_the_request_is_marked() {
    let r = rig();
    r.up.route(HOST, "/depot/1/chunk/a", FakeResponse { status: 302, ..FakeResponse::ok(b"", &[("location", "/m")]) });
    r.up.route(HOST, "/m", FakeResponse::ok(&body(40), &[]));
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/a");
    let Known::Fetching { total, first, .. } = r.fetcher.learn(&obj, 0, None) else { panic!() };
    assert_eq!(total, Some(40));
    first.wait_end().unwrap();
    r.settle();
    let seen = r.up.seen.lock().unwrap().clone();
    assert_eq!(seen.iter().map(|s| s.target.as_str()).collect::<Vec<_>>(), ["/depot/1/chunk/a", "/m"]);
    assert!(seen.iter().all(|s| s.headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(VIA_HEADER))));
    assert!(matches!(r.store.lookup(&obj.key), Lookup::Found(_)));
    assert_eq!(r.fetcher.counters.stored_records.load(Ordering::Relaxed), 1);
}

#[test]
fn a_body_that_fails_its_check_is_not_stored() {
    let r = rig();
    let good = crc32(&body(30)).to_string();
    r.up.route(HOST, "/depot/1/chunk/ok", FakeResponse::ok(&body(30), &[("x-content-crc", &good)]));
    r.up.route(HOST, "/depot/1/chunk/bad", FakeResponse::ok(&body(30), &[("x-content-crc", "1")]));
    for (target, stored) in [("/depot/1/chunk/ok", true), ("/depot/1/chunk/bad", false)] {
        let obj = object(Arc::new(support::profiles::Chunks), target);
        let Known::Fetching { first, .. } = r.fetcher.learn(&obj, 0, None) else { panic!() };
        let _ = first.wait_end();
        r.settle();
        assert_eq!(matches!(r.store.lookup(&obj.key), Lookup::Found(_)), stored, "{target}");
    }
    assert_eq!(r.fetcher.counters.crc_mismatch.load(Ordering::Relaxed), 1);
}

#[test]
fn a_redirect_is_followed_only_to_a_host_cached_for() {
    use lanstash_domain::service::hosts::HostRules;
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 1 << 20, alignment: 4096 }, read_only: false };
    let (store, _) = Store::open(Roots::single(Arc::new(MemFs::default())), opts, Arc::new(FixedClock(1))).unwrap();
    let up = Arc::new(lanstash_app::test_support::FakeUpstream::default());
    let mut hosts = HostRules::default();
    hosts.add_host("steam", "cdn.example");
    let fetcher = Fetcher::new(up.clone(), Arc::new(store), Arc::new(NoEvents), FetchOptions::default());
    let fetcher = Arc::new(fetcher.with_hosts(hosts));
    let inside =
        FakeResponse { status: 302, ..FakeResponse::ok(b"", &[("location", "http://10.0.0.1:8081/shutdown")]) };
    up.route(HOST, "/depot/1/chunk/r", inside);
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/r");
    assert!(matches!(fetcher.learn(&obj, 0, None), Known::Failed(e) if e.contains("not cached for")));
    assert_eq!(up.calls(), 1, "the address inside the network was never asked");
}

struct Panics;

impl Upstream for Panics {
    fn fetch(&self, _: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError> {
        panic!("an origin adapter that panics");
    }
}

#[test]
fn a_download_that_panics_wakes_its_waiters_and_leaves_the_table() {
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 1 << 20, alignment: 4096 }, read_only: false };
    let (store, _) = Store::open(Roots::single(Arc::new(MemFs::default())), opts, Arc::new(FixedClock(1))).unwrap();
    let fetcher =
        Arc::new(Fetcher::new(Arc::new(Panics), Arc::new(store), Arc::new(NoEvents), FetchOptions::default()));
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/p");
    assert!(matches!(fetcher.learn(&obj, 0, None), Known::Failed(_)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fetcher.in_flight() > 0 {
        assert!(std::time::Instant::now() < until, "the download stayed in the table");
        std::thread::yield_now();
    }
}

#[test]
fn an_origin_error_is_relayed_and_not_stored() {
    let r = rig();
    r.up.route(HOST, "/depot/1/chunk/gone", FakeResponse { status: 404, ..FakeResponse::ok(b"no", &[]) });
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/gone");
    let Known::Relay { download, .. } = r.fetcher.learn(&obj, 0, None) else { panic!("not relayed") };
    let _ = download.wait_end();
    r.settle();
    assert!(matches!(r.store.lookup(&obj.key), Lookup::Missing));
}
