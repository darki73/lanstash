//! Battle.net builds: the patch service's current build of each game in the
//! stash, its pre-release build, and how much of each install the stash holds.

mod support;

use lanstash_app::test_support::FakeResponse;
use lanstash_battlenet_domain::tact::PATCH_HOST;
use lanstash_ports::prefill::Prefill as _;
use support::*;

#[test]
fn a_game_is_current_only_when_its_install_is_stashed() {
    let w = bnet(false);
    // The launcher fetched the build config; that alone is not the build.
    w.get(&at("config", BUILD, ""), None);
    let first = w.builds.check_all();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].version().map(|v| v.name.as_str()), Some("3.3.7"));
    assert!(!first[0].current(), "coverage unknown until planned");
    let c = w.planned();
    let cov = c.build.as_ref().unwrap().coverage.unwrap();
    assert!(cov.have > 0 && cov.have < cov.want, "{cov:?}");
    assert!(!c.current());
    w.prefill.start("osi", false, lanstash_ports::prefill::PrefillFrom::Cdn).unwrap();
    while w.prefill.status().unwrap().state == lanstash_ports::prefill::PrefillState::Running {
        std::thread::yield_now();
    }
    w.settle();
    let after = w.builds.check("osi");
    assert!(after.current(), "{after:?}");
    assert!(after.prerelease.is_none());
}

#[test]
fn a_pre_release_build_is_reported_with_its_own_coverage() {
    let w = bnet(true);
    w.get(&at("config", BUILD, ""), None);
    let c = w.planned();
    let pre = c.prerelease.expect("bgdl names another build");
    assert_eq!((pre.version.build_config.as_str(), pre.version.name.as_str()), (PRE_BUILD, "3.4.0"));
    assert!(pre.coverage.is_some_and(|c| !c.full()));
}

#[test]
fn a_pre_release_of_the_current_build_is_none() {
    let w = bnet(false);
    let same = "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0\n\
                ## seqn = 1\n";
    let rows = format!("{same}eu|{BUILD}|{CDN_CONFIG}||7|3.3.7\n");
    w.up.route(PATCH_HOST, "/osi/bgdl", FakeResponse::ok(rows.as_bytes(), &[]));
    w.get(&at("config", BUILD, ""), None);
    assert!(w.planned().prerelease.is_none());
}

#[test]
fn launcher_data_and_unknown_games() {
    let w = bnet(false);
    assert!(w.builds.skipped("bnt004") && w.builds.skipped("catalogs") && !w.builds.skipped("osi"));
    let none = w.builds.check("nothing");
    assert!(none.build.is_none() && none.product.is_none());
}

#[test]
fn tables_are_kept_for_a_while() {
    let w = bnet(false);
    let _ = w.builds.check("osi");
    let n = w.up.seen.lock().unwrap().iter().filter(|r| r.host == PATCH_HOST && r.target == "/osi/versions").count();
    let _ = w.builds.check("osi");
    let again =
        w.up.seen.lock().unwrap().iter().filter(|r| r.host == PATCH_HOST && r.target == "/osi/versions").count();
    assert_eq!((n, again), (1, 1));
    w.clock.0.fetch_add(11 * 60_000, std::sync::atomic::Ordering::SeqCst);
    let _ = w.builds.check("osi");
    let later =
        w.up.seen.lock().unwrap().iter().filter(|r| r.host == PATCH_HOST && r.target == "/osi/versions").count();
    assert_eq!(later, 2);
}
