//! Cover art: fetched once through the upstream, kept in memory, bounded.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_app::art::{Art, ArtOptions};
use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::Source;

const STEAM_ART_HOST: &str = "art.example";

/// Headers of numeric ids at a fixed address, as Steam's are.
struct Fixed;

impl ArtSource for Fixed {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        let numeric = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit());
        match kind {
            ArtKind::Header if numeric => Ok(Source { host: STEAM_ART_HOST.into(), port: None, target: header(id) }),
            _ => Err(ArtError::Unknown),
        }
    }
}

#[derive(Default)]
struct Tick(AtomicU64);
impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A JPEG's first bytes, then `n` more.
fn jpeg(n: usize) -> Vec<u8> {
    let mut b = b"\xFF\xD8\xFF\xE0".to_vec();
    b.resize(4 + n, 0);
    b
}

fn ok(body: &[u8]) -> FakeResponse {
    FakeResponse::ok(body, &[("Content-Type", "image/jpeg")])
}

fn art(opts: ArtOptions) -> (Arc<FakeUpstream>, Arc<Tick>, Art) {
    let up = Arc::new(FakeUpstream::default());
    let clock = Arc::new(Tick::default());
    let a = Art::new(up.clone(), clock.clone(), opts).with_source("steam", Arc::new(Fixed));
    (up, clock, a)
}

fn header(app: &str) -> String {
    format!("/steam/apps/{app}/header.jpg")
}

const H: ArtKind = ArtKind::Header;

#[test]
fn fetches_once_then_serves_from_memory() {
    let (up, _, a) = art(ArtOptions::default());
    up.route(STEAM_ART_HOST, &header("1422450"), ok(&jpeg(6)));
    let img = a.get("steam", "1422450", H).unwrap();
    assert_eq!((img.content_type, img.body.len()), ("image/jpeg", 10));
    assert_eq!(a.get("steam", "1422450", H).unwrap(), img);
    assert_eq!(up.calls(), 1);
    let seen = up.seen.lock().unwrap()[0].clone();
    assert_eq!((seen.method.as_str(), seen.host.as_str()), ("GET", STEAM_ART_HOST));
    assert_eq!(a.held(), 10);
}

#[test]
fn unknown_platforms_kinds_and_bad_ids_never_reach_the_upstream() {
    let (up, _, a) = art(ArtOptions::default());
    assert_eq!(a.get("blizzard", "osi", H), Err(ArtError::Unknown), "no source given");
    assert_eq!(a.get("steam", "../etc", H), Err(ArtError::Unknown));
    assert_eq!(a.get("steam", "1", ArtKind::Icon), Err(ArtError::Unknown), "Steam has no icon here");
    assert_eq!(up.calls(), 0);
    assert_eq!(
        (ArtKind::parse(None), ArtKind::parse(Some("icon")), ArtKind::parse(Some("x"))),
        (Some(H), Some(ArtKind::Icon), None)
    );
}

#[test]
fn a_miss_is_remembered_for_a_while() {
    let opts = ArtOptions { miss_ttl_ms: 1_000, ..ArtOptions::default() };
    let (up, clock, a) = art(opts);
    up.route(STEAM_ART_HOST, &header("7"), FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    assert_eq!(a.get("steam", "7", H), Err(ArtError::Missing));
    assert_eq!(a.get("steam", "7", H), Err(ArtError::Missing));
    assert_eq!(up.calls(), 1);
    clock.0.store(1_000, Ordering::SeqCst);
    assert_eq!(a.get("steam", "7", H), Err(ArtError::Missing));
    assert_eq!(up.calls(), 2, "asked again after the miss expired");
}

/// A hero that may be missing, and another picture to fall back on.
struct WithFallback;

impl ArtSource for WithFallback {
    fn locate(&self, id: &str, _kind: ArtKind) -> Result<Source, ArtError> {
        Ok(Source { host: STEAM_ART_HOST.into(), port: None, target: format!("/{id}/hero.jpg") })
    }
    fn fallback(&self, id: &str, _kind: ArtKind) -> Option<Source> {
        Some(Source { host: STEAM_ART_HOST.into(), port: None, target: format!("/{id}/page.jpg") })
    }
}

#[test]
fn a_missing_image_is_looked_for_where_the_source_says_else() {
    let up = Arc::new(FakeUpstream::default());
    let a = Art::new(up.clone(), Arc::new(Tick::default()), ArtOptions::default())
        .with_source("steam", Arc::new(WithFallback));
    let missing = || FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) };
    up.route(STEAM_ART_HOST, "/1/hero.jpg", ok(&jpeg(3)));
    up.route(STEAM_ART_HOST, "/1/page.jpg", ok(&jpeg(9)));
    assert_eq!(a.get("steam", "1", H).unwrap().body.len(), 7, "the hero when there is one");
    assert_eq!(up.calls(), 1, "the other picture is not asked for then");
    up.route(STEAM_ART_HOST, "/2/hero.jpg", missing());
    up.route(STEAM_ART_HOST, "/2/page.jpg", ok(&jpeg(9)));
    assert_eq!(a.get("steam", "2", H).unwrap().body.len(), 13, "the other picture when the hero is missing");
    up.route(STEAM_ART_HOST, "/3/hero.jpg", missing());
    up.route(STEAM_ART_HOST, "/3/page.jpg", missing());
    assert_eq!(a.get("steam", "3", H), Err(ArtError::Missing));
    assert_eq!(Fixed.fallback("1", H), None, "a source names none unless it has one");
}

#[test]
fn failures_are_retried_sooner_and_unusable_answers_refused() {
    let opts = ArtOptions { fail_ttl_ms: 10, max_image: 8, ..ArtOptions::default() };
    let (up, clock, a) = art(opts);
    assert!(matches!(a.get("steam", "1", H), Err(ArtError::Upstream(_))), "connection refused");
    assert!(matches!(a.get("steam", "1", H), Err(ArtError::Upstream(_))));
    assert_eq!(up.calls(), 1);
    clock.0.store(10, Ordering::SeqCst);
    up.route(STEAM_ART_HOST, &header("1"), ok(&jpeg(1)));
    assert!(a.get("steam", "1", H).is_ok());
    up.route(STEAM_ART_HOST, &header("2"), ok(b"<svg onload=x>"[..6].as_ref()));
    assert!(
        matches!(a.get("steam", "2", H), Err(ArtError::Upstream(e)) if e.contains("not an image")),
        "the type is from the bytes"
    );
    up.route(STEAM_ART_HOST, &header("3"), ok(&jpeg(20)));
    assert!(matches!(a.get("steam", "3", H), Err(ArtError::Upstream(e)) if e.contains("over 8 bytes")));
    up.route(STEAM_ART_HOST, &header("4"), FakeResponse { chunked: true, ..ok(&jpeg(20)) });
    assert!(
        matches!(a.get("steam", "4", H), Err(ArtError::Upstream(e)) if e.contains("over 8 bytes")),
        "no length given"
    );
    up.route(STEAM_ART_HOST, &header("5"), FakeResponse { status: 503, ..ok(b"") });
    assert!(matches!(a.get("steam", "5", H), Err(ArtError::Upstream(e)) if e.contains("503")));
}

#[test]
fn drops_the_least_recently_used_image_past_the_budget() {
    let opts = ArtOptions { max_total: 10, ..ArtOptions::default() };
    let (up, _, a) = art(opts);
    for app in ["1", "2", "3"] {
        up.route(STEAM_ART_HOST, &header(app), ok(&jpeg(1)));
    }
    a.get("steam", "1", H).unwrap();
    a.get("steam", "2", H).unwrap();
    a.get("steam", "1", H).unwrap(); // 2 is now the oldest
    a.get("steam", "3", H).unwrap();
    assert_eq!((a.held(), up.calls()), (10, 3));
    a.get("steam", "1", H).unwrap();
    assert_eq!(up.calls(), 3, "1 was kept");
    a.get("steam", "2", H).unwrap();
    assert_eq!(up.calls(), 4, "2 was dropped");
}
