//! Plans: what an install of a Battle.net build downloads (the download
//! manifest's files for the chosen tags, where they lie), and how much of it
//! the stash holds.

#[path = "../support/mod.rs"]
mod support;

mod held;
mod make;

use lanstash_battlenet_domain::tact::Version;
use support::*;

fn version() -> Version {
    Version { build_config: BUILD.into(), cdn_config: CDN_CONFIG.into(), name: "3.3.7".into() }
}

#[test]
fn plans_are_kept_and_made_in_the_background() {
    let w = bnet(false);
    assert!(w.plans.ready("osi", &version()).is_none(), "not made yet: queued");
    let check = w.planned();
    assert!(check.build.unwrap().coverage.is_some());
    let calls = w.up.calls();
    assert!(w.plans.ready("osi", &version()).unwrap().is_ok());
    let _ = w.plans.plan("osi", &version()).unwrap();
    assert_eq!(w.up.calls(), calls, "kept, not made again");
}

#[test]
fn another_installs_plan_is_made_in_the_background_too() {
    let w = bnet(false);
    assert!(w.plans.set_up("windows") && !w.plans.set_up("OSX"), "the install set up is Windows");
    assert!(w.plans.ready_for("osi", &version(), &["OSX"]).is_none(), "not made yet: queued");
    let start = std::time::Instant::now();
    let mac = loop {
        if let Some(made) = w.plans.ready_for("osi", &version(), &["OSX"]) {
            break made.unwrap();
        }
        assert!(start.elapsed().as_secs() < 10, "the plan was never made");
        std::thread::yield_now();
    };
    let usual = w.plans.plan("osi", &version()).unwrap();
    assert_ne!(mac.parts, usual.parts, "the Mac's plan, kept apart from the usual one");
}

#[test]
fn the_indexes_spread_over_the_hosts_whichever_worker_takes_them() {
    // One worker takes every index: each still goes to its own host in turn.
    let w = bnet_planning(1);
    let _ = w.plans.plan("osi", &version()).unwrap();
    let index_hosts: std::collections::BTreeSet<String> =
        w.up.seen.lock().unwrap().iter().filter(|r| r.target.ends_with(".index")).map(|r| r.host.clone()).collect();
    assert_eq!(index_hosts.len(), 2, "{index_hosts:?}");
}

#[test]
fn a_planning_says_its_step_and_a_plan_waiting_its_turn_is_queued() {
    use lanstash_battlenet_app::plans::key;
    use lanstash_ports::prefill::{PlanStep, Planning};
    use std::time::{Duration, Instant};
    let w = bnet(false);
    w.up.hold_matching(".index");
    assert!(w.plans.ready("osi", &version()).is_none());
    let until = Instant::now() + Duration::from_secs(5);
    let step = || w.plans.progress(BUILD).and_then(|p| p.step);
    while step() != Some(PlanStep::Indexes) && Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(
        w.plans.progress(BUILD),
        Some(Planning { done: 2, total: 7, step: Some(PlanStep::Indexes) }),
        "both configs in, five indexes asked for"
    );
    // One plan is made at a time: another install's waits for this one.
    assert!(w.plans.ready_for("osi", &version(), &["OSX"]).is_none());
    let mac = key(&version(), &["OSX"]);
    assert_eq!(w.plans.progress(&mac), Some(Planning { done: 0, total: 0, step: Some(PlanStep::Queued) }));
    w.up.release();
    while w.plans.ready_for("osi", &version(), &["OSX"]).is_none() && Instant::now() < until {
        std::thread::yield_now();
    }
    assert!(w.plans.ready("osi", &version()).is_some_and(|m| m.is_ok()));
    assert_eq!((w.plans.progress(BUILD), w.plans.progress(&mac)), (None, None), "made: no planning left");
}

#[test]
fn a_build_is_planned_once_at_a_time() {
    use lanstash_ports::prefill::{PlanStep, Planning};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    let w = bnet(false);
    assert_eq!(w.plans.progress(BUILD), None, "not being planned");
    w.up.hold();
    let first = {
        let plans = Arc::clone(&w.plans);
        std::thread::spawn(move || plans.plan("osi", &version()))
    };
    let until = Instant::now() + Duration::from_secs(5);
    while w.plans.progress(BUILD).is_none() && Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(
        w.plans.progress(BUILD),
        Some(Planning { done: 0, total: 0, step: Some(PlanStep::Configs) }),
        "the CDN table is held: the configs are next"
    );
    // A second caller waits for the first instead of planning again.
    let second = {
        let plans = Arc::clone(&w.plans);
        std::thread::spawn(move || plans.plan("osi", &version()))
    };
    assert_eq!(w.plans.ready("osi", &version()), None, "not ready while being planned");
    std::thread::sleep(Duration::from_millis(20));
    w.up.release();
    let (a, b) = (first.join().unwrap().unwrap(), second.join().unwrap().unwrap());
    assert!(Arc::ptr_eq(&a, &b), "one plan, made once");
    assert_eq!(w.plans.progress(BUILD), None);
    // The indexes went out over both of the region's hosts.
    let index_hosts: std::collections::BTreeSet<String> =
        w.up.seen.lock().unwrap().iter().filter(|r| r.target.ends_with(".index")).map(|r| r.host.clone()).collect();
    assert_eq!(index_hosts.len(), 2, "{index_hosts:?}");
}
