//! The job line: one job at a time, the queued ones after it, each ended job told once.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use lanstash_app::prefill::{Planner, Work};
use lanstash_app::test_support::FakeResponse;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, StartError};

use super::{CDN, Fixed, job, wait_all, world};

#[test]
fn every_job_that_ended_is_told_once() {
    let w = world();
    w.up.route(CDN, "/f", FakeResponse::ok(b"0123456789", &[]));
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/f", None)])));
    w.up.hold();
    for g in ["a", "b", "c"] {
        p.queue(g, false, PrefillFrom::Cdn).unwrap();
    }
    w.up.release();
    wait_all(&w, &p);
    let ended: Vec<_> = p.take_ended().into_iter().map(|s| (s.game, s.state)).collect();
    let done = PrefillState::Done;
    assert_eq!(ended, [("a".into(), done), ("b".into(), done), ("c".into(), done)]);
    assert!(p.take_ended().is_empty());
}

/// Counts the jobs being planned at once.
#[derive(Default)]
struct Overlap {
    active: AtomicUsize,
    most: AtomicUsize,
}

impl Planner for Overlap {
    fn admit(&self, _game: &str) -> Result<(), StartError> {
        Ok(())
    }
    fn work(&self, _game: &str, _pre: bool, _version: &dyn Fn(&str)) -> Result<Work, String> {
        let n = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(n, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(1));
        self.active.fetch_sub(1, Ordering::SeqCst);
        Err("nothing to do".into())
    }
}

#[test]
fn a_start_between_two_queued_jobs_does_not_run_beside_them() {
    let w = world();
    let planner = Arc::new(Overlap::default());
    let p = w.prefiller(planner.clone());
    for i in 0..200 {
        let _ = p.queue(&format!("a{i}"), false, PrefillFrom::Cdn);
        let _ = p.queue(&format!("b{i}"), false, PrefillFrom::Cdn);
        // The moment the first job is seen ended, try to start another.
        let until = Instant::now() + Duration::from_secs(5);
        while p.status().is_some_and(|s| s.state == PrefillState::Running) && Instant::now() < until {
            std::hint::spin_loop();
        }
        let _ = p.start(&format!("x{i}"), false, PrefillFrom::Cdn);
        wait_all(&w, &p);
    }
    assert_eq!(planner.most.load(Ordering::SeqCst), 1, "one job at a time");
}
