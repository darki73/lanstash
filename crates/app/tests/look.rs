//! Tests for `lanstash_app::look`: going on as soon as the mark moves, else
//! after the longest wait.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_app::look::until_changed;
use lanstash_ports::clock::Pause;

/// A pause that only adds up how long it was asked to wait.
#[derive(Default)]
struct Counted(AtomicU64);

impl Pause for Counted {
    fn pause_ms(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::Relaxed);
    }
}

#[test]
fn it_goes_on_as_soon_as_the_mark_moves() {
    let pause = Counted::default();
    let looks = Mutex::new(0);
    let mark = || {
        let mut n = looks.lock().unwrap();
        *n += 1;
        u64::from(*n >= 3)
    };
    assert!(until_changed(&pause, 30_000, 300_000, &0, &mark));
    assert_eq!(pause.0.load(Ordering::Relaxed), 90_000, "three looks of 30 s");
}

#[test]
fn it_stops_waiting_after_the_longest_wait() {
    let pause = Counted::default();
    assert!(!until_changed(&pause, 30_000, 100_000, &7, &|| 7));
    assert_eq!(pause.0.load(Ordering::Relaxed), 100_000, "the last look is shorter");
}
