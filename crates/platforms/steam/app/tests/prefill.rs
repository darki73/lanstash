//! Tests for `lanstash_steam_app::prefill`, against a scripted account and a fake CDN.

mod support;

use std::sync::{Arc, Mutex};

use lanstash_app::prefill::Planner;
use lanstash_domain::checksum::crc::crc32;
use lanstash_domain::segment::meta::encode_meta;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::prefill::StartError;
use lanstash_steam_app::library::SteamLibrary;
use lanstash_steam_app::prefill::SteamPlanner;
use lanstash_steam_domain::apps::Target;
use support::account::services;

fn planner(signed_in: bool) -> SteamPlanner {
    let (s, _) = services(signed_in);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    SteamPlanner::new(s, lib, Target::default())
}

#[test]
fn an_install_is_every_unique_chunk_of_its_depots() {
    let p = planner(true);
    assert_eq!(p.admit("100"), Ok(()));
    let seen = Mutex::new(Vec::new());
    let w = p.work("100", false, &|v| seen.lock().unwrap().push(v.to_string())).unwrap();
    assert_eq!(*seen.lock().unwrap(), ["555"]);
    assert_eq!(w.hosts, ["lancache.steamcontent.com"]);
    let targets: Vec<String> = w.jobs.iter().map(|j| j.target.clone()).collect();
    assert_eq!(
        targets,
        [
            format!("/depot/101/chunk/{}", "01".repeat(20)),
            format!("/depot/101/chunk/{}", "02".repeat(20)),
            format!("/depot/102/chunk/{}", "03".repeat(20)),
        ]
    );
    assert_eq!(w.bytes, 340, "the Linux depot is not taken");
    assert!(w.jobs.iter().all(|j| j.range.is_none()) && (w.parts)(0).is_empty());
    assert!(p.planning().is_none(), "planning is over");
    // From lancache: the CRC-32 lancache kept with the chunk, and the manifest's size.
    let body = vec![7u8; 100];
    let crc = crc32(&body).to_string();
    let mut good = (w.check)(0, 0, 100, &encode_meta(&[("x-content-crc", &crc)]));
    good.feed(&body[..40]);
    good.feed(&body[40..]);
    assert!(good.finish());
    let mut short = (w.check)(0, 0, 100, &encode_meta(&[("x-content-crc", &crc)]));
    short.feed(&body[..99]);
    assert!(!short.finish());
    let mut bare = (w.check)(0, 0, 100, &[]);
    bare.feed(&body);
    assert!(!bare.finish(), "no x-content-crc: fetched from the CDN instead");
}

#[test]
fn what_cannot_be_prefilled_says_why() {
    assert_eq!(planner(false).admit("100"), Err(StartError::NotSetUp));
    let p = planner(true);
    assert_eq!(p.admit("not-a-number"), Err(StartError::NotAGame));
    p.work("100", false, &|_| {}).unwrap(); // reads the library
    assert_eq!(p.admit("7"), Err(StartError::NotAGame), "not in the library");
    assert!(p.work("100", true, &|_| {}).unwrap_err().contains("no pre-release"));
    assert_eq!(p.work("7", false, &|_| {}).unwrap_err(), "app 7 is not in the account's library");
}

#[test]
fn each_depot_is_read_as_the_account_its_license_grants_it_to() {
    let asked = support::two_accounts::Asked::default();
    let s = support::two_accounts::services(asked.clone());
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    let p = SteamPlanner::new(s, lib, Target::default());
    let w = p.work("100", false, &|_| {}).unwrap();
    let depots: std::collections::BTreeSet<&str> =
        w.jobs.iter().map(|j| j.target.split('/').nth(2).unwrap_or_default()).collect();
    assert_eq!(depots, ["101", "102"].into(), "one account's depot and the other's, in one install");
    let mut codes = asked.lock().unwrap().clone();
    codes.sort();
    assert_eq!(
        codes,
        [("player_one".to_string(), 101), ("player_two".to_string(), 102)],
        "no code asked of the wrong one"
    );
}

#[test]
fn another_operating_system_takes_its_own_depots() {
    let p = planner(true);
    assert!(p.installs());
    let w = p.work_for("100", false, Some("linux"), &|_| {}).unwrap();
    let targets: Vec<String> = w.jobs.iter().map(|j| j.target.clone()).collect();
    assert_eq!(
        targets,
        [
            format!("/depot/101/chunk/{}", "01".repeat(20)),
            format!("/depot/101/chunk/{}", "02".repeat(20)),
            format!("/depot/103/chunk/{}", "04".repeat(20)),
        ],
        "the depot for every system and Linux's, not Windows'"
    );
    assert_eq!(p.work_for("100", false, Some("macos"), &|_| {}).unwrap().jobs.len(), 2, "only the shared depot");
    assert_eq!(
        p.work_for("100", false, Some("amiga"), &|_| {}).unwrap_err(),
        "amiga is not an operating system Steam installs for"
    );
}
