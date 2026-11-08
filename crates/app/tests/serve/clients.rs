//! Tests for `lanstash_app::serve::clients`: the bytes sent to each client per game,
//! from the stash and while downloading, now and since the start.

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use lanstash_app::serve::clients::{ClientCounter, ClientTraffic, PREFIX};
use lanstash_app::test_support::Capture;
use lanstash_ports::clock::Clock;

struct Ticking(AtomicU64);

impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

fn get_from(cache: &Cache, ip: &str, host: &str, target: &str, range: &str) -> Capture {
    let mut c = Capture::default();
    let mut conn = ConnState::new(false).with_client(ip.parse().unwrap());
    cache.handle(&request("GET", host, target, &[("range", range)]), &mut conn, &mut c).unwrap();
    c
}

#[test]
fn each_clients_bytes_are_counted_per_game_from_the_stash_or_not() {
    let clock = Arc::new(Ticking(AtomicU64::new(50_000)));
    let traffic = Arc::new(ClientTraffic::new(clock.clone(), 10_000));
    let mut r = rig();
    r.cache = r.cache.with_clients(Arc::clone(&traffic));
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    assert_eq!(get_from(&r.cache, "10.0.0.5", BNET, BNET_PATH, "bytes=0-199").status, 206);
    r.settle();
    assert_eq!(get_from(&r.cache, "::ffff:10.0.0.5", BNET, BNET_PATH, "bytes=0-99").status, 206);
    assert_eq!(get_from(&r.cache, "10.0.0.6", BNET, BNET_PATH, "bytes=0-49").status, 206);
    let counters = traffic.counters();
    let named: Vec<(ClientCounter, u64)> =
        counters.iter().map(|(n, v)| (ClientCounter::parse(n).expect(n), *v)).collect();
    let of = |client: &str, hit: bool| named.iter().find(|(c, _)| c.client == client && c.hit == hit).map(|(_, v)| *v);
    assert_eq!(of("10.0.0.5", false), Some(200), "sent while downloading");
    assert_eq!(of("10.0.0.5", true), Some(100), "from the stash; a mapped address is the IPv4 one");
    assert_eq!(of("10.0.0.6", true), Some(50));
    assert_eq!(named.len(), 3, "{counters:?}");
    assert!(named.iter().all(|(c, _)| c.service == "blizzard"));
    assert_eq!(traffic.clients(), ["10.0.0.5", "10.0.0.6"]);
}

#[test]
fn a_connection_without_an_address_is_not_counted() {
    let traffic = Arc::new(ClientTraffic::new(Arc::new(Ticking(AtomicU64::new(1))), 10_000));
    let mut r = rig();
    r.cache = r.cache.with_clients(Arc::clone(&traffic));
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(100), &[]));
    assert_eq!(r.get(BNET, BNET_PATH, Some("bytes=0-9")).status, 206);
    assert!(traffic.counters().is_empty());
    assert_eq!(r.cache.clients().map(|c| c.clients().len()), Some(0));
}

#[test]
fn now_is_the_last_seconds_and_fastest_first() {
    let clock = Arc::new(Ticking(AtomicU64::new(100_000)));
    let t = ClientTraffic::new(clock.clone(), 10_000);
    t.add("10.0.0.5", "steam", "7", true, 4_000);
    t.add("10.0.0.6", "steam", "8", false, 50_000);
    clock.0.store(105_000, Ordering::Relaxed);
    t.add("10.0.0.5", "steam", "7", false, 6_000);
    t.add("10.0.0.5", "steam", "9", true, 0);
    let now = t.now();
    assert_eq!(now.len(), 2, "nothing sent is not downloading: {now:?}");
    assert_eq!((now[0].client.as_str(), now[0].per_sec), ("10.0.0.6", 5_000), "50 kB over 10 s");
    assert_eq!((now[1].hit_bytes, now[1].miss_bytes, now[1].total_bytes), (4_000, 6_000, 10_000));
    clock.0.store(112_000, Ordering::Relaxed);
    let now = t.now();
    assert_eq!(now.len(), 1, "10.0.0.6 sent nothing for 12 s");
    assert_eq!((now[0].hit_bytes, now[0].miss_bytes), (0, 6_000), "the first second fell out of the window");
    clock.0.store(200_000, Ordering::Relaxed);
    assert!(t.now().is_empty());
    assert_eq!(t.counters().len(), 3, "since the start: kept");
}

#[test]
fn a_download_after_a_pause_starts_again_and_the_total_goes_on() {
    use lanstash_app::serve::clients::SESSION_GAP_MS;
    let clock = Arc::new(Ticking(AtomicU64::new(1_000_000)));
    let t = ClientTraffic::new(clock.clone(), 10_000);
    t.add("10.0.0.5", "blizzard", "d3", true, 5_000);
    clock.0.store(1_000_000 + SESSION_GAP_MS, Ordering::Relaxed);
    t.add("10.0.0.5", "blizzard", "d3", true, 1_000);
    let now = &t.now()[0];
    assert_eq!((now.session_bytes, now.since_ms), (6_000, 1_000_000), "a pause of the gap itself: the same download");
    let later = 1_000_000 + 2 * SESSION_GAP_MS + 1;
    clock.0.store(later, Ordering::Relaxed);
    t.add("10.0.0.5", "blizzard", "d3", true, 3_000);
    let now = &t.now()[0];
    assert_eq!((now.session_bytes, now.since_ms, now.total_bytes), (3_000, later, 9_000), "deleted, fetched anew");
}

#[test]
fn a_counter_name_reads_back() {
    let c = ClientCounter::parse(&format!("{PREFIX}10.0.0.5/steam/7/hit")).unwrap();
    assert_eq!((c.client.as_str(), c.service.as_str(), c.game.as_str(), c.hit), ("10.0.0.5", "steam", "7", true));
    let v6 = ClientCounter::parse("client/fd00::5/blizzard/tpr/hs/miss").unwrap();
    assert_eq!((v6.client.as_str(), v6.game.as_str(), v6.hit), ("fd00::5", "tpr/hs", false));
    for bad in ["served/steam/7", "client/10.0.0.5/steam/7/other", "client/10.0.0.5/steam/hit", "client//steam/7/hit"] {
        assert_eq!(ClientCounter::parse(bad), None, "{bad}");
    }
}
