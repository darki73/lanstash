//! Tests for `lanstash_app::serve::object`: an object's length and headers,
//! checked bodies, and clients sharing a download.

use super::*;

#[test]
fn ranged_miss_then_hit() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[("content-type", "application/octet-stream")]));

    let c = r.get(BNET, BNET_PATH, Some("bytes=100-299"));
    assert_eq!((c.status, c.header("x-lanstash")), (206, Some("MISS")));
    assert_eq!(c.header("content-range"), Some("bytes 100-299/1000"));
    assert_eq!(c.body, &data[100..300]);
    r.settle();
    // Exactly 100..300, in pieces of 128 bytes: nothing beyond what the client asked for.
    let mut seen = r.ranges_seen();
    seen.sort();
    assert_eq!(seen, vec![Some("bytes=100-227".to_owned()), Some("bytes=228-299".to_owned())]);

    let calls = r.up.calls();
    let c = r.get(BNET, BNET_PATH, Some("bytes=150-250"));
    assert_eq!((c.status, c.header("x-lanstash")), (206, Some("HIT")));
    assert_eq!(c.header("content-type"), Some("application/octet-stream"));
    assert_eq!(c.body, &data[150..251]);
    assert_eq!(r.up.calls(), calls, "a hit must not reach the origin");
}

#[test]
fn every_answer_says_it_came_through_a_lan_cache() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(100), &[]));
    r.up.route(BNET, "/tpr/hs/x.version", FakeResponse::ok(b"v1", &[]));
    let marks = |c: &Capture| {
        let h = |n: &str| c.header(n).map(str::to_owned);
        (h("x-lancache-processed-by").as_deref() == Some("lanstash"), h("x-upstream-cache-status"))
    };
    let miss = r.get(BNET, BNET_PATH, None);
    assert_eq!(marks(&miss), (true, Some("MISS".into())));
    r.settle();
    assert_eq!(marks(&r.get(BNET, BNET_PATH, None)), (true, Some("HIT".into())));
    assert_eq!(marks(&r.get(BNET, "/tpr/hs/x.version", None)), (true, Some("BYPASS".into())));
}

#[test]
fn steam_chunk_with_valid_crc_is_stored() {
    let r = rig();
    let data = body(300);
    let crc = crc32(&data).to_string();
    r.up.route(STEAM, STEAM_PATH, FakeResponse::ok(&data, &[("x-content-crc", &crc)]));
    let c = r.get(STEAM, STEAM_PATH, None);
    assert_eq!(c.body, data);
    assert_eq!(c.header("x-content-crc"), Some(crc.as_str()));
    r.settle();
    let c = r.get(STEAM, STEAM_PATH, Some("bytes=10-19"));
    assert_eq!((c.status, c.header("x-lanstash")), (206, Some("HIT")));
    assert_eq!(c.header("x-content-crc"), None, "whole-body headers do not belong on a part");
    assert_eq!(c.body, &data[10..20]);
    assert_eq!(r.up.calls(), 1);
}

#[test]
fn steam_chunk_with_bad_crc_is_not_completed_nor_stored() {
    let r = rig();
    r.up.route(STEAM, STEAM_PATH, FakeResponse::ok(&body(300), &[("x-content-crc", "12345")]));
    let (res, c) = r.send(&request("GET", STEAM, STEAM_PATH, &[]));
    assert!(res.is_err(), "the client must not receive a complete corrupt body");
    assert!(c.body.len() < 300);
    r.settle();
    let (_, _) = r.send(&request("GET", STEAM, STEAM_PATH, &[]));
    assert!(r.up.calls() >= 2, "a corrupt body must not be served from storage");
}

#[test]
fn concurrent_clients_share_one_download() {
    let r = Arc::new(rig());
    let data = body(100);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    r.up.hold();
    let clients: Vec<_> = (0..4)
        .map(|_| {
            let r = Arc::clone(&r);
            std::thread::spawn(move || r.get(BNET, BNET_PATH, None).body)
        })
        .collect();
    std::thread::sleep(Duration::from_millis(50));
    r.up.release();
    for c in clients {
        assert_eq!(c.join().unwrap(), data);
    }
    assert_eq!(r.up.calls(), 1);
}

/// A clock the test moves.
struct Moving(std::sync::atomic::AtomicU64);
impl lanstash_ports::clock::Clock for Moving {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[test]
fn lanstash_reading_its_own_stash_does_not_count_as_a_use() {
    let up = Arc::new(FakeUpstream::default());
    let b = body(300);
    up.route(BNET, BNET_PATH, FakeResponse::ranged(&b, &[]));
    let clock = Arc::new(Moving(std::sync::atomic::AtomicU64::new(1_000)));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let mut hosts = HostRules::default();
    hosts.add_host("blizzard", "blizzard.com");
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), fetch_opts()));
    let pass = Arc::new(Passthrough::new(up.clone(), 1 << 20, 4096));
    let serve = ServeOptions { piece_bytes: 50, prefetch: true, retries: 3 };
    let cache = Cache::new(Arc::clone(&store), hosts, &support::profiles::profiles(), Arc::new(NoEvents), serve)
        .with_upstream(fetcher, pass);
    let get = |agent: &str| {
        let req = request("GET", BNET, BNET_PATH, &[("user-agent", agent)]);
        let mut c = Capture::default();
        cache.handle(&req, &mut ConnState::default(), &mut c).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while cache.fetcher().unwrap().in_flight() > 0 && Instant::now() < until {
            std::thread::yield_now();
        }
    };
    let used = || store.usage().values().map(|u| u.last_used_ms).max().unwrap_or(0);
    get("Battle.net Agent");
    assert_eq!(used(), 1_000, "downloaded and stored at 1 000");
    clock.0.store(9_000, Ordering::SeqCst);
    get(lanstash_app::via_cache::USER_AGENT);
    assert_eq!(used(), 1_000, "a plan or prefill reading it leaves its last use alone");
    get("Battle.net Agent");
    assert_eq!(used(), 9_000, "a client reading it is a use");
}
