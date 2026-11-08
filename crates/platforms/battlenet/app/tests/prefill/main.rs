//! Prefill: a Battle.net build's install goes into the stash through the
//! cache's own request path, as the download manifest and archive indexes
//! say; what is stashed already is skipped.

#[path = "../support/mod.rs"]
mod support;

mod lancache;

use std::time::{Duration, Instant};

use lanstash_domain::extent::Span;
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, PrefillStatus, StartError};
use support::*;

fn wait(w: &Bnet) -> PrefillStatus {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = w.prefill.status().unwrap();
        if s.state != PrefillState::Running || Instant::now() > until {
            w.settle();
            return s;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn stores_the_install_and_nothing_else() {
    let w = bnet(false);
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    let s = wait(&w);
    assert_eq!((s.state, s.error.as_deref(), s.version.as_deref()), (PrefillState::Done, None, Some("3.3.7")));
    // 9 whole files (7 read while planning, from the CDN alone: the stash held none of the game) and 2
    // archive ranges, all fetched and stashed now.
    assert_eq!((s.files_total, s.files_done, s.files_skipped, s.files_missing), (11, 11, 0, 0), "{s:?}");
    assert!(s.bytes_total > X_SIZE + Z_SIZE + L_SIZE + ENC_SIZE);
    assert!(s.bytes >= X_SIZE + Z_SIZE + L_SIZE + ENC_SIZE);
    assert!(w.holds(&at("data", ARCHIVE_1, ""), Some(Span::new(w.x_at, w.x_at + X_SIZE))));
    assert!(w.holds(&at("data", ARCHIVE_2, ""), None));
    assert!(w.holds(&at("data", &key(L), ""), None) && w.holds(&at("data", &key(ENC), ""), None));
    // Y (OSX only) was not asked for: no request reaches past X.
    let a1 = at("data", ARCHIVE_1, "");
    for (t, r) in w.asked() {
        if t == a1 {
            let r = r.expect("archives are read in ranges");
            let end: u64 = r.rsplit('-').next().unwrap().parse().unwrap();
            assert!(end < w.x_at + X_SIZE, "{r}");
        }
    }
    // A second run finds everything stashed.
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    let again = wait(&w);
    assert_eq!((again.state, again.files_skipped, again.bytes), (PrefillState::Done, 11, 0));
    assert_eq!(again.bytes_skipped, s.bytes, "what the first run took, the second finds stashed");
}

#[test]
fn the_pre_release_build() {
    let w = bnet(true);
    w.prefill.start("osi", true, PrefillFrom::Cdn).unwrap();
    let s = wait(&w);
    assert_eq!((s.state, s.version.as_deref(), s.prerelease), (PrefillState::Done, Some("3.4.0"), true), "{s:?}");
    assert!(w.holds(&at("config", PRE_BUILD, ""), None));
    let none = bnet(false);
    none.prefill.start("osi", true, PrefillFrom::Cdn).unwrap();
    let f = wait(&none);
    assert_eq!(f.state, PrefillState::Failed);
    assert!(f.error.as_deref().is_some_and(|e| e.contains("pre-release")), "{f:?}");
}

#[test]
fn one_at_a_time_and_stops_when_asked() {
    let w = bnet(false);
    w.up.hold();
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    assert_eq!(w.prefill.start("osi", false, PrefillFrom::Cdn), Err(StartError::Busy));
    w.prefill.stop();
    w.up.release();
    let s = wait(&w);
    assert_eq!(s.state, PrefillState::Stopped, "{s:?}");
    assert!(s.files_done < s.files_total || s.files_total == 0);
    assert!(s.finished_ms.is_some());
}

#[test]
fn the_launchers_own_data_is_not_a_game() {
    let w = bnet(false);
    assert_eq!(w.prefill.start("bnt004", false, PrefillFrom::Cdn), Err(StartError::NotAGame));
    assert_eq!(w.prefill.start("catalogs", false, PrefillFrom::Cdn), Err(StartError::NotAGame));
    assert!(w.prefill.status().is_none());
}

#[test]
fn an_unknown_game_fails_with_a_reason() {
    let w = bnet(false);
    w.prefill.start("nothing", false, PrefillFrom::Cdn).unwrap();
    let s = wait(&w);
    assert_eq!(s.state, PrefillState::Failed);
    assert!(s.error.is_some(), "{s:?}");
}

/// Requests to the CDN once the plan is made: the 7 documents planning read (from the CDN alone,
/// the stash holding none of the game), ENC, L and the two archive ranges, all independent, so
/// `parallel` of them (at most 11) are in flight at once.
fn in_flight_at_once(parallel: usize) -> usize {
    use lanstash_app::prefill::PrefillOptions;
    let w = bnet_with(false, PrefillOptions { parallel, ..PrefillOptions::default() });
    let version = w.builds.check("osi").build.unwrap().version;
    w.plans.plan("osi", &version).unwrap();
    w.settle();
    let before = w.up.calls();
    w.up.hold();
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    // Wait for the workers to block on the held CDN, then a little longer for any more.
    let until = Instant::now() + Duration::from_secs(5);
    while w.up.calls() < before + parallel.min(11) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(50));
    let at_once = w.up.calls() - before;
    w.up.release();
    let s = wait(&w);
    assert_eq!((s.state, s.files_done, s.files_skipped), (PrefillState::Done, 11, 0), "{s:?}");
    at_once
}

#[test]
fn requests_go_out_in_parallel() {
    assert_eq!(in_flight_at_once(1), 1);
    assert_eq!(in_flight_at_once(3), 3);
    assert_eq!(in_flight_at_once(8), 8);
    assert_eq!(in_flight_at_once(16), 11, "no more than there are requests");
}

#[test]
fn the_status_says_how_far_planning_has_come() {
    let w = bnet(false);
    let _ = w.builds.check("osi"); // the version is known; the plan is not
    w.up.hold();
    w.prefill.start("osi", false, PrefillFrom::Cdn).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    let planning = loop {
        let s = w.prefill.status().unwrap();
        if s.planning.is_some() || Instant::now() > until {
            break s.planning;
        }
        std::thread::yield_now();
    };
    assert!(planning.is_some_and(|p| p.done <= p.total), "{planning:?}");
    w.up.release();
    let s = wait(&w);
    assert_eq!((s.state, s.planning), (PrefillState::Done, None), "{s:?}");
}
