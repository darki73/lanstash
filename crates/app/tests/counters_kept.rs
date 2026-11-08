//! Tests for `lanstash_app::counters_kept`: what went up is kept by the minute,
//! a restart's counters count from 0, gauges are left out, and a series is
//! asked in buckets that fit.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::counters_kept::{CounterSource, CountersKept, KeptOptions, MAX_BUCKETS, step};
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::clock::Clock;

const MIN: u64 = 60_000;

struct Ticking(AtomicU64);

impl Clock for Ticking {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

fn kept(values: &Arc<Mutex<Vec<(String, u64)>>>, clock: &Arc<Ticking>) -> CountersKept {
    let v = Arc::clone(values);
    let source: CounterSource = Arc::new(move || v.lock().unwrap().clone());
    CountersKept::new(source, Arc::new(MemoryLogs::default()), clock.clone(), KeptOptions::default())
}

fn set(values: &Mutex<Vec<(String, u64)>>, now: &[(&str, u64)]) {
    *values.lock().unwrap() = now.iter().map(|(n, v)| ((*n).to_owned(), *v)).collect();
}

#[test]
fn what_went_up_each_minute_is_kept() {
    let values = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(Ticking(AtomicU64::new(MIN)));
    let k = kept(&values, &clock);
    set(&values, &[("requests.hits", 5), ("eviction.stored_bytes", 900), ("lifetime.served_bytes", 77)]);
    k.sample().unwrap();
    clock.0.store(2 * MIN, Ordering::Relaxed);
    set(&values, &[("requests.hits", 12), ("eviction.stored_bytes", 950)]);
    k.sample().unwrap();
    // A restart: the counter starts again from 0; what it shows now all counts.
    clock.0.store(3 * MIN, Ordering::Relaxed);
    set(&values, &[("requests.hits", 3)]);
    k.sample().unwrap();
    let names = ["requests.hits".to_string(), "eviction.stored_bytes".to_string(), "lifetime.served_bytes".to_string()];
    assert_eq!(
        k.series(&names, 0, 4 * MIN, MIN).unwrap(),
        vec![(0, vec![5, 0, 0]), (MIN, vec![7, 0, 0]), (2 * MIN, vec![3, 0, 0])],
        "a sample counts for the minute that ended; gauges and lifetime totals are not kept"
    );
}

#[test]
fn a_long_series_is_asked_in_buckets_that_fit() {
    assert_eq!(step(0, 60 * MIN, MIN), MIN);
    assert_eq!(step(0, 60 * MIN, 90_000), 2 * MIN, "whole minutes");
    let year = 365 * 24 * 60 * MIN;
    assert!(year / step(0, year, MIN) <= MAX_BUCKETS);
}
