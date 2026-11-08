//! Tests for `lanstash_app::rounds`.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use lanstash_app::rounds::{AFTER_PANIC_MS, Next, forever};
use lanstash_ports::clock::Pause;

#[derive(Default)]
struct Paused(AtomicU64);

impl Pause for Paused {
    fn pause_ms(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

#[test]
fn a_round_that_panics_is_followed_by_the_next_after_a_pause() {
    let paused = Arc::new(Paused::default());
    let rounds = Arc::new(AtomicUsize::new(0));
    let (done, ended) = mpsc::channel();
    let r = Arc::clone(&rounds);
    forever("test-rounds", Arc::clone(&paused) as Arc<dyn Pause>, move || {
        let n = r.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            panic!("round {n} fails");
        }
        let _ = done.send(());
        Next::Stop
    })
    .unwrap();
    ended.recv_timeout(Duration::from_secs(5)).expect("the loop went on after the panics");
    assert_eq!(rounds.load(Ordering::SeqCst), 3);
    assert_eq!(paused.0.load(Ordering::SeqCst), 2 * AFTER_PANIC_MS);
}
