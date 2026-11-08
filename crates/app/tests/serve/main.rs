//! Tests for `lanstash_app::serve`, end to end through the request handler:
//! fake origin, in-memory storage.

mod body;
mod clients;
mod object;
mod refused;
mod served;
#[path = "../support/mod.rs"]
mod support;
mod tally;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use lanstash_app::fetch::{FetchOptions, Fetcher};
use lanstash_app::passthrough::Passthrough;
use lanstash_app::serve::{Cache, ConnState, RequestHandler, ServeOptions};
use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream};
use lanstash_domain::checksum::crc::crc32;
use lanstash_domain::http::RequestHead;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_store::game::SegmentOptions;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

const BNET: &str = "level3.blizzard.com";
const BNET_PATH: &str = "/tpr/hs/data/8e/fc/8efccb8531df1859cffa8f081a8eddd1";
const STEAM: &str = "cache1.steamcontent.com";
const STEAM_PATH: &str = "/depot/7/chunk/0123456789abcdef0123456789abcdef01234567";

struct Rig {
    fs: MemFs,
    up: Arc<FakeUpstream>,
    cache: Cache,
}

fn body(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251) as u8).collect()
}

fn fetch_opts() -> FetchOptions {
    FetchOptions {
        piece: 128,
        ahead: 2,
        window: 64,
        object_file_min: 4096,
        read_bytes: 4096,
        max_unknown_length: 1 << 20,
    }
}

fn rig_on(fs: MemFs, up: Arc<FakeUpstream>, opts: FetchOptions) -> Rig {
    let store_opts =
        StoreOptions { segments: SegmentOptions { max_bytes: 1 << 20, alignment: 4096 }, read_only: false };
    let (store, _) = Store::open(Roots::single(Arc::new(fs.clone())), store_opts, Arc::new(FixedClock(1_000))).unwrap();
    let store = Arc::new(store);
    let mut hosts = HostRules::default();
    hosts.add_host("blizzard", "blizzard.com");
    hosts.add_host("steam", "steamcontent.com");
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), opts));
    let pass = Arc::new(Passthrough::new(up.clone(), 1 << 20, 4096));
    let serve = ServeOptions { piece_bytes: 50, prefetch: true, retries: 3 };
    let cache = Cache::new(store, hosts, &support::profiles::profiles(), Arc::new(NoEvents), serve)
        .with_upstream(fetcher, pass);
    Rig { fs, up, cache }
}

fn rig() -> Rig {
    rig_on(MemFs::default(), Arc::new(FakeUpstream::default()), fetch_opts())
}

fn request(method: &str, host: &str, target: &str, extra: &[(&str, &str)]) -> RequestHead {
    let mut headers = vec![("host".to_owned(), host.to_owned())];
    headers.extend(extra.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())));
    RequestHead { method: method.into(), target: target.into(), minor_version: 1, headers }
}

impl Rig {
    fn send(&self, req: &RequestHead) -> (Result<(), String>, Capture) {
        let mut c = Capture::default();
        let r = self.cache.handle(req, &mut ConnState::default(), &mut c).map_err(|e| e.to_string());
        (r, c)
    }

    fn get(&self, host: &str, target: &str, range: Option<&str>) -> Capture {
        let extra: Vec<(&str, &str)> = range.map(|r| ("range", r)).into_iter().collect();
        let (r, c) = self.send(&request("GET", host, target, &extra));
        r.unwrap();
        c
    }

    /// Waits until no download is running (stores are committed by then).
    fn settle(&self) {
        let f = self.cache.fetcher().unwrap();
        let t = Instant::now();
        while f.in_flight() > 0 {
            assert!(t.elapsed() < Duration::from_secs(5), "downloads did not finish");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn ranges_seen(&self) -> Vec<Option<String>> {
        self.up
            .seen
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("range")).map(|(_, v)| v.clone()))
            .collect()
    }
}

#[test]
fn range_past_the_end_is_416() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(100), &[]));
    let c = r.get(BNET, BNET_PATH, Some("bytes=500-"));
    assert_eq!((c.status, c.header("content-range")), (416, Some("bytes */100")));
}

#[test]
fn uncacheable_paths_pass_through() {
    let r = rig();
    r.up.route(BNET, "/tpr/hs/x.version", FakeResponse::ok(b"v1", &[]));
    let c = r.get(BNET, "/tpr/hs/x.version", None);
    assert_eq!((c.status, c.header("x-lanstash"), c.body.as_slice()), (200, Some("PASS"), &b"v1"[..]));
    r.get(BNET, "/tpr/hs/x.version", None);
    assert_eq!(r.up.calls(), 2);
}

#[test]
fn a_path_spelled_with_dot_segments_passes_and_stores_nothing() {
    let r = rig();
    let odd = "/tpr/x1/../hs/data/8e/fc/8efccb8531df1859cffa8f081a8eddd1";
    r.up.route(BNET, odd, FakeResponse::ok(b"v1", &[]));
    let c = r.get(BNET, odd, None);
    assert_eq!((c.status, c.header("x-lanstash")), (200, Some("PASS")));
    assert!(r.fs.paths().iter().all(|p| !p.contains("/seg/") && !p.contains("/obj/")), "{:?}", r.fs.paths());
}

#[test]
fn control_answers() {
    let r = rig();
    assert_eq!(r.get("anything", "/lancache-heartbeat", None).status, 204);
    assert_eq!(r.get("example.org", "/", None).status, 403);
    assert_eq!(r.send(&request("POST", BNET, BNET_PATH, &[])).1.status, 405);
    assert_eq!(r.send(&request("GET", BNET, BNET_PATH, &[("x-lanstash-via", "1")])).1.status, 508);
    assert_eq!(r.up.calls(), 0);
}

#[test]
fn head_sends_no_body_but_the_length() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(700), &[]));
    let (res, c) = r.send(&request("HEAD", BNET, BNET_PATH, &[]));
    res.unwrap();
    assert_eq!((c.status, c.content_length, c.body.len()), (200, 700, 0));
}
