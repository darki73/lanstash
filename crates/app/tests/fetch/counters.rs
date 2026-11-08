//! Tests for `lanstash_app::fetch::counters`: how the origin answered, counted (error statuses,
//! timeouts, the wait for its response head).

use std::sync::atomic::AtomicU64;

use lanstash_ports::clock::Clock;
use lanstash_ports::upstream::{Upstream, UpstreamError, UpstreamRequest, UpstreamResponse};

use super::*;

/// A clock 25 ms further on at every look.
#[derive(Default)]
struct Ticking(AtomicU64);

impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(25, Ordering::Relaxed)
    }
}

/// An origin that never answers in time.
struct Slow;

impl Upstream for Slow {
    fn fetch(&self, _: &UpstreamRequest<'_>) -> Result<Box<dyn UpstreamResponse>, UpstreamError> {
        Err(UpstreamError::timeout("receive: timed out"))
    }
}

fn n(a: &AtomicU64) -> u64 {
    a.load(Ordering::Relaxed)
}

fn fetcher(up: Arc<dyn Upstream>) -> Fetcher {
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 1 << 20, alignment: 4096 }, read_only: false };
    let (store, _) = Store::open(Roots::single(Arc::new(MemFs::default())), opts, Arc::new(FixedClock(1))).unwrap();
    Fetcher::new(up, Arc::new(store), Arc::new(NoEvents), FetchOptions::default())
}

fn settle(f: &Fetcher) {
    let until = Instant::now() + Duration::from_secs(5);
    while f.in_flight() > 0 {
        assert!(Instant::now() < until, "the download stayed in the table");
        std::thread::yield_now();
    }
}

#[test]
fn error_statuses_and_the_wait_for_each_answer_are_counted() {
    let up = Arc::new(FakeUpstream::default());
    let f = Arc::new(fetcher(up.clone()).with_clock(Arc::new(Ticking::default())));
    for (path, status) in [("/depot/1/chunk/a", 404), ("/depot/1/chunk/b", 503), ("/depot/1/chunk/c", 200)] {
        up.route(HOST, path, FakeResponse { status, ..FakeResponse::ok(b"body", &[]) });
        let obj = object(Arc::new(support::profiles::Chunks), path);
        let _ = f.learn(&obj, 0, None);
        settle(&f);
    }
    let k = &f.counters;
    assert_eq!((n(&k.upstream_4xx), n(&k.upstream_5xx), n(&k.timeouts)), (1, 1, 0));
    assert_eq!((n(&k.answers_timed), n(&k.answer_wait_ms)), (3, 75), "25 ms each, by the clock");
}

#[test]
fn an_origin_that_does_not_answer_in_time_is_a_timeout() {
    let f = Arc::new(fetcher(Arc::new(Slow)));
    let obj = object(Arc::new(support::profiles::Chunks), "/depot/1/chunk/slow");
    assert!(matches!(f.learn(&obj, 0, None), Known::Failed(_)));
    settle(&f);
    let k = &f.counters;
    assert_eq!((n(&k.timeouts), n(&k.failed), n(&k.answers_timed)), (1, 1, 0), "no answer: no wait counted");
}
