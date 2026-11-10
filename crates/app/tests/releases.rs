//! Tests for `lanstash_app::releases`: the newest release, asked about in the background now and
//! then, and `lanstash doctor`'s line for it.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lanstash_app::health::Level;
use lanstash_app::releases::{ReleaseWatch, WatchOptions, version_check};
use lanstash_ports::clock::Clock;
use lanstash_ports::releases::Releases;

/// Answers with what it is told, counting the asks.
#[derive(Default)]
struct Told {
    answer: Mutex<Option<Result<Option<String>, String>>>,
    asked: AtomicUsize,
}

impl Told {
    fn says(&self, a: Result<Option<&str>, &str>) {
        *self.answer.lock().unwrap() = Some(a.map(|v| v.map(str::to_owned)).map_err(str::to_owned));
    }
}

impl Releases for Told {
    fn latest(&self) -> Result<Option<String>, String> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.answer.lock().unwrap().clone().unwrap_or(Ok(None))
    }
}

struct Ticking(AtomicU64);
impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

const OPTS: WatchOptions = WatchOptions { every_ms: 1_000, retry_ms: 100 };

fn world() -> (Arc<Told>, Arc<Ticking>, ReleaseWatch) {
    let told = Arc::new(Told::default());
    let clock = Arc::new(Ticking(AtomicU64::new(10_000)));
    let watch = ReleaseWatch::new(told.clone(), clock.clone(), OPTS);
    (told, clock, watch)
}

/// Asks until the watch has asked `n` times in all, giving its thread time.
fn settle(watch: &ReleaseWatch, told: &Told, n: usize) -> Option<String> {
    let until = Instant::now() + Duration::from_secs(5);
    let mut out = watch.newer_than("0.1.0");
    while told.asked.load(Ordering::SeqCst) < n && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
        out = watch.newer_than("0.1.0");
    }
    std::thread::sleep(Duration::from_millis(20));
    watch.newer_than("0.1.0").or(out)
}

#[test]
fn a_newer_release_is_learnt_in_the_background_and_kept() {
    let (told, clock, watch) = world();
    told.says(Ok(Some("0.2.0")));
    assert_eq!(watch.newer_than("0.1.0"), None, "nothing learnt yet: no one waits for the ask");
    assert_eq!(settle(&watch, &told, 1), Some("0.2.0".into()));
    assert_eq!(watch.newer_than("0.2.0"), None, "not newer than itself");
    assert_eq!(watch.newer_than("0.3.0-dev"), None, "not newer than a build ahead of it");
    assert_eq!(told.asked.load(Ordering::SeqCst), 1, "kept: not asked again before it is due");
    clock.0.fetch_add(1_000, Ordering::SeqCst);
    told.says(Ok(Some("0.3.0")));
    let _ = watch.newer_than("0.1.0");
    assert_eq!(settle(&watch, &told, 2), Some("0.3.0".into()), "due again: asked again");
}

#[test]
fn a_failed_ask_is_tried_again_sooner_and_says_nothing() {
    let (told, clock, watch) = world();
    told.says(Err("api.github.com: HTTP 403"));
    assert_eq!(settle(&watch, &told, 1), None);
    clock.0.fetch_add(100, Ordering::SeqCst);
    told.says(Ok(Some("0.2.0")));
    let _ = watch.newer_than("0.1.0");
    assert_eq!(settle(&watch, &told, 2), Some("0.2.0".into()), "retried after retry_ms, not every_ms");
}

#[test]
fn doctor_says_whether_a_newer_release_is_out() {
    let told = Told::default();
    told.says(Ok(Some("0.2.0")));
    let c = version_check("0.1.0", &told);
    assert_eq!((c.level, c.detail.as_str()), (Level::Warn, "0.1.0; 0.2.0 is available"));
    assert_eq!(version_check("0.2.0", &told).level, Level::Ok);
    assert_eq!(version_check("0.3.0-dev", &told).detail, "0.3.0-dev (the latest)", "ahead is not behind");
    told.says(Ok(None));
    assert!(version_check("0.1.0", &told).detail.contains("not checked"));
    told.says(Err("offline"));
    let c = version_check("0.1.0", &told);
    assert_eq!(c.level, Level::Warn);
    assert!(c.detail.contains("offline"));
}
