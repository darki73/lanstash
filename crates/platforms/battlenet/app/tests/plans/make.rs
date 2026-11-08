//! Making a plan: the download manifest's files for the chosen tags, and
//! where they lie.

use lanstash_battlenet_domain::locate::Range;
use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::extent::Span;

use super::support::*;
use super::version;

#[test]
fn an_install_is_the_manifests_files_for_the_tags() {
    let w = bnet(false);
    let plan = w.plans.plan("osi", &version()).unwrap();
    let names: Vec<&str> = plan.files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        names,
        vec![
            at("config", BUILD, ""),
            at("config", CDN_CONFIG, ""),
            at("data", ARCHIVE_1, ".index"),
            at("data", ARCHIVE_2, ".index"),
            at("data", FILE_INDEX, ".index"),
            at("patch", PATCH_ARCHIVE, ".index"),
            at("patch", PATCH_FILE_INDEX, ".index"),
            at("data", &key(ENC), ""),
            at("data", &key(L), ""),
        ]
    );
    assert_eq!(plan.files[7].1, ENC_SIZE, "loose sizes from the file index");
    assert_eq!(plan.archives, vec![at("data", ARCHIVE_1, ""), at("data", ARCHIVE_2, "")]);
    assert_eq!((plan.host.as_str(), plan.hosts.clone()), (CDN, vec![CDN.to_owned(), CDN_2.to_owned()]));
    // The manifest (archive 1 at 0) and X; Y is OSX only; Z in archive 2.
    let dl = w.x_at - 10;
    assert_eq!(
        plan.parts,
        vec![
            (0, Span::new(0, dl), key(DL)),
            (0, Span::new(w.x_at, w.x_at + X_SIZE), key(X)),
            (1, Span::new(0, Z_SIZE), key(Z)),
        ]
    );
    assert_eq!(
        plan.requests,
        vec![Range { archive: 0, start: 0, end: w.x_at + X_SIZE }, Range { archive: 1, start: 0, end: Z_SIZE }],
        "the manifest and X are 10 bytes apart: one request"
    );
    assert_eq!(plan.absent, 2, "the VFS file and W are on no CDN");
    let files: u64 = plan.files.iter().map(|f| f.1).sum();
    assert_eq!(plan.bytes(), files + dl + X_SIZE + Z_SIZE);
}

#[test]
fn another_platforms_install_has_a_plan_of_its_own() {
    let w = bnet(false);
    let usual = w.plans.plan("osi", &version()).unwrap();
    assert_eq!(usual.installs.platforms, ["Windows", "OSX"], "the platforms the build has installs for");
    let mac = w.plans.plan_for("osi", &version(), &["OSX"]).unwrap();
    let keys = |p: &lanstash_battlenet_domain::plan::Plan| p.parts.iter().map(|x| x.2.clone()).collect::<Vec<_>>();
    assert!(keys(&mac).contains(&key(Y)) && !keys(&mac).contains(&key(X)), "the Mac's files, not Windows'");
    assert!(keys(&usual).contains(&key(X)) && !keys(&usual).contains(&key(Y)), "kept apart from the usual plan");
    assert!(w.plans.plan_for("osi", &version(), &["Switch"]).is_err(), "a platform the build has no install for");
}

#[test]
fn a_build_whose_files_are_missing_has_no_plan() {
    let w = bnet(false);
    let gone = Version { build_config: "b".repeat(32), cdn_config: CDN_CONFIG.into(), name: "0".into() };
    let e = w.plans.plan("osi", &gone).unwrap_err();
    assert!(e.contains("/config/"), "{e}");
}
