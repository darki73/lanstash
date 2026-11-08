//! Tests for `lanstash_app::serve::served`: the bytes sent to clients per game,
//! from the stash and while downloading, named for the counters kept by the minute.

use super::*;
use lanstash_app::serve::served::{PREFIX, ServedByGame};

#[test]
fn bytes_sent_are_counted_per_game() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[("content-type", "application/octet-stream")]));
    assert_eq!(r.get(BNET, BNET_PATH, Some("bytes=100-299")).status, 206);
    r.settle();
    assert_eq!(r.get(BNET, BNET_PATH, Some("bytes=150-249")).status, 206);
    let counters = r.cache.served.counters();
    assert_eq!(counters.len(), 1, "{counters:?}");
    let (name, bytes) = &counters[0];
    assert!(name.starts_with(&format!("{PREFIX}blizzard/")), "{name}");
    assert_eq!(*bytes, 300, "200 while downloading, then 100 from the stash");
}

#[test]
fn lanstash_s_own_requests_count_apart_from_clients() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    let get = request("GET", BNET, BNET_PATH, &[("range", "bytes=0-399")]);
    let mut c = Capture::default();
    r.cache.handle(&get, &mut ConnState::own(), &mut c).unwrap();
    assert_eq!(c.status, 206);
    r.settle();
    r.cache.handle(&get, &mut ConnState::own(), &mut Capture::default()).unwrap();
    let n = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
    let k = &r.cache.counters;
    assert_eq!([n(&k.own_requests), n(&k.own_bytes)], [2, 800]);
    assert_eq!([n(&k.requests), n(&k.hits), n(&k.misses), n(&k.hit_bytes), n(&k.miss_bytes)], [0; 5]);
    assert!(r.cache.served.counters().is_empty(), "no game was served to a client");
    // What it fetched from the internet is still fetched.
    assert_eq!(n(&r.cache.fetcher().unwrap().counters.fetched_bytes), 400);
}

#[test]
fn nothing_sent_is_not_counted() {
    let s = ServedByGame::default();
    s.add("steam", "7", 0);
    assert!(s.counters().is_empty());
    s.add("steam", "7", 5);
    s.add("steam", "7", 6);
    s.add("steam", "8", 1);
    assert_eq!(s.counters(), [("served/steam/7".to_string(), 11), ("served/steam/8".to_string(), 1)]);
}
