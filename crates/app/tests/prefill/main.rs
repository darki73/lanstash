//! The prefill runner: one job at a time, of the requests a platform's
//! planner gives, each through the cache or out of lancache's cache.

#[path = "../support/mod.rs"]
mod support;

mod asked;
mod checked;
mod line;
mod run;
mod tail;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lanstash_app::fetch::{FetchOptions, Fetcher};
use lanstash_app::lancache_read::{LancacheOptions, LancacheRead};
use lanstash_app::passthrough::Passthrough;
use lanstash_app::prefill::{Job, PieceCheck, Planner, PrefillOptions, Prefiller, Work};
use lanstash_app::serve::{Cache, ServeOptions};
use lanstash_app::stash_put::StashPut;
use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, PrefillStatus, StartError};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use lanstash_store::test_support::nginx::MemNginx;
use support::profiles::{Ranged, profiles};

/// A check that passes or fails whatever it is fed.
struct Verdict(bool);

impl PieceCheck for Verdict {
    fn feed(&mut self, _data: &[u8]) {}
    fn finish(&self) -> bool {
        self.0
    }
}

/// The CDN host of the tests.
pub const CDN: &str = "cdn.example";

/// Jobs of the `game` directory: given requests, or an error; bytes accepted when `accept`.
#[derive(Debug)]
pub struct Fixed {
    pub jobs: Vec<Job>,
    pub fail: Option<String>,
    pub accept: Arc<AtomicBool>,
    pub admitted: Mutex<Vec<String>>,
    /// The parts of each job (by job number), for taking part of a range.
    pub parts: Vec<Vec<(u64, u64)>>,
    /// What the CDN sends is checked ([`Work::check_cdn`]).
    pub check_cdn: bool,
}

impl Fixed {
    pub fn new(jobs: Vec<Job>) -> Self {
        Self {
            jobs,
            fail: None,
            accept: Arc::new(AtomicBool::new(true)),
            admitted: Mutex::default(),
            parts: Vec::new(),
            check_cdn: false,
        }
    }
}

impl Planner for Fixed {
    fn admit(&self, game: &str) -> Result<(), StartError> {
        self.admitted.lock().unwrap().push(game.into());
        match game {
            "launcher" => Err(StartError::NotAGame),
            "unplanned" => Err(StartError::NotSetUp),
            _ => Ok(()),
        }
    }

    fn work(&self, _game: &str, _prerelease: bool, version: &dyn Fn(&str)) -> Result<Work, String> {
        version("1.0");
        if let Some(e) = &self.fail {
            return Err(e.clone());
        }
        let accept = Arc::clone(&self.accept);
        let self_parts = self.parts.clone();
        Ok(Work {
            hosts: vec![CDN.into()],
            jobs: self.jobs.clone(),
            bytes: 30,
            check: Box::new(move |_, _, _, _| Box::new(Verdict(accept.load(Ordering::SeqCst)))),
            parts: Box::new(move |i| self_parts.get(i).cloned().unwrap_or_default()),
            check_cdn: self.check_cdn,
        })
    }
}

#[derive(Debug)]
pub struct World {
    pub up: Arc<FakeUpstream>,
    pub store: Arc<Store>,
    pub cache: Arc<Cache>,
}

pub fn world() -> World {
    let clock = Arc::new(FixedClock(5));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let up = Arc::new(FakeUpstream::default());
    let mut hosts = HostRules::default();
    hosts.add_host("blizzard", CDN);
    let fetcher = Arc::new(Fetcher::new(up.clone(), Arc::clone(&store), Arc::new(NoEvents), FetchOptions::default()));
    let pass = Arc::new(Passthrough::new(up.clone(), 1 << 20, 4096));
    let cache = Cache::new(Arc::clone(&store), hosts, &profiles(), Arc::new(NoEvents), ServeOptions::default());
    World { up, store, cache: Arc::new(cache.with_upstream(fetcher, pass)) }
}

impl World {
    pub fn prefiller(&self, planner: Arc<dyn Planner>) -> Prefiller {
        let clock = Arc::new(FixedClock(9));
        let opts = PrefillOptions { parallel: 2, ..PrefillOptions::default() };
        Prefiller::new(planner, "blizzard", self.cache.clone(), Arc::clone(&self.store), clock, opts)
    }

    /// [`World::prefiller`] with lancache's cache `nginx` (1 KiB slices).
    pub fn with_lancache(&self, planner: Arc<dyn Planner>, nginx: &Arc<MemNginx>) -> Prefiller {
        let opts = LancacheOptions { service: "blizzard".into(), slice_bytes: 1024, max_piece: 1 << 20 };
        let read = LancacheRead::new(Arc::clone(nginx) as _, opts);
        let put = StashPut::new(Arc::clone(&self.store), Arc::new(Ranged), 1 << 20);
        self.prefiller(planner).with_lancache(read, put)
    }

    /// [`World::prefiller`] whose CDN downloads are checked before they are stored.
    pub fn with_checked(&self, planner: Arc<dyn Planner>) -> Prefiller {
        let put = StashPut::new(Arc::clone(&self.store), Arc::new(Ranged), 1 << 20);
        self.prefiller(planner).with_checked_cdn(self.up.clone(), put)
    }

    pub fn wait(&self, p: &Prefiller) -> PrefillStatus {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let s = p.status().unwrap();
            if s.state != PrefillState::Running || Instant::now() > until {
                while self.cache.fetcher().unwrap().in_flight() > 0 {
                    std::thread::yield_now();
                }
                return s;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

pub fn job(target: &str, range: Option<(u64, u64)>) -> Job {
    Job { target: target.into(), range }
}

#[test]
fn the_planner_admits_a_game_first() {
    let w = world();
    let p = w.prefiller(Arc::new(Fixed::new(Vec::new())));
    assert_eq!(p.status(), None);
    assert!(!p.lancache());
    assert_eq!(p.start("launcher", false, PrefillFrom::Cdn), Err(StartError::NotAGame));
    assert_eq!(p.start("unplanned", false, PrefillFrom::Cdn), Err(StartError::NotSetUp));
    assert_eq!(p.start("osi", false, PrefillFrom::Lancache), Err(StartError::NoLancache));
    assert_eq!(p.status(), None, "nothing started");
}

#[test]
fn one_job_at_a_time() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/a", FakeResponse::ok(b"0123456789", &[]));
    w.up.hold();
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/tpr/osi/a", None)])));
    p.start("osi", true, PrefillFrom::Cdn).unwrap();
    assert_eq!(p.start("osi", false, PrefillFrom::Cdn), Err(StartError::Busy));
    w.up.release();
    let s = w.wait(&p);
    assert_eq!((s.game.as_str(), s.prerelease, s.from, s.state), ("osi", true, PrefillFrom::Cdn, PrefillState::Done));
    assert_eq!((s.version.as_deref(), s.started_ms, s.finished_ms), (Some("1.0"), 9, Some(9)));
}

#[test]
fn stopping_ends_the_job_as_stopped() {
    let w = world();
    w.up.route(CDN, "/tpr/osi/a", FakeResponse::ok(b"0123456789", &[]));
    w.up.hold();
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/tpr/osi/a", None), job("/tpr/osi/b", None)])));
    p.start("osi", false, PrefillFrom::Cdn).unwrap();
    p.stop();
    w.up.release();
    assert_eq!(w.wait(&p).state, PrefillState::Stopped);
}

/// Waits until no job runs and the queue is empty.
fn wait_all(w: &World, p: &Prefiller) -> PrefillStatus {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = w.wait(p);
        if s.queued.is_empty() && s.state != PrefillState::Running || Instant::now() > until {
            return s;
        }
        std::thread::yield_now();
    }
}

#[test]
fn queued_jobs_run_in_turn_once_each() {
    let w = world();
    w.up.route(CDN, "/f", FakeResponse::ok(b"0123456789", &[]));
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/f", None)])));
    w.up.hold();
    assert_eq!(p.queue("a", false, PrefillFrom::Cdn), Ok(0), "nothing runs: it starts");
    assert_eq!(p.queue("b", false, PrefillFrom::Cdn), Ok(1));
    assert_eq!(p.queue("c", false, PrefillFrom::Cdn), Ok(2));
    assert_eq!(p.queue("b", false, PrefillFrom::Cdn), Ok(2), "already queued: not twice");
    assert_eq!(p.queue("a", false, PrefillFrom::Cdn), Ok(2), "the running one: not again");
    assert_eq!(p.queue("launcher", false, PrefillFrom::Cdn), Err(StartError::NotAGame));
    assert_eq!(p.status().unwrap().queued, ["b", "c"]);
    assert_eq!(p.queue("d", false, PrefillFrom::Cdn), Ok(3));
    assert!(p.unqueue("d") && !p.unqueue("d") && !p.unqueue("a"), "only a queued one, once");
    w.up.release();
    let s = wait_all(&w, &p);
    assert_eq!((s.game.as_str(), s.state), ("c", PrefillState::Done), "the last one ran last");
}

#[test]
fn stopping_forgets_the_queue() {
    let w = world();
    w.up.route(CDN, "/f", FakeResponse::ok(b"0123456789", &[]));
    let p = w.prefiller(Arc::new(Fixed::new(vec![job("/f", None)])));
    w.up.hold();
    p.queue("a", false, PrefillFrom::Cdn).unwrap();
    p.queue("b", false, PrefillFrom::Cdn).unwrap();
    p.stop();
    assert!(p.status().unwrap().queued.is_empty());
    w.up.release();
    let s = w.wait(&p);
    assert_eq!((s.game.as_str(), s.state), ("a", PrefillState::Stopped));
}
