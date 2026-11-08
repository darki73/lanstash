//! How much of a plan the stash holds.

use super::support::*;
use super::version;

/// A client fetched X: the stash holds the game's directory, so planning stashes what it reads.
fn holding_x() -> Bnet {
    let w = bnet(false);
    w.get(&at("data", ARCHIVE_1, ""), Some(&format!("bytes={}-{}", w.x_at, w.x_at + X_SIZE - 1)));
    w
}

#[test]
fn looking_at_a_game_the_stash_does_not_hold_stashes_nothing() {
    let w = bnet(false);
    let plan = w.plans.plan("osi", &version()).unwrap();
    w.settle();
    assert_eq!(w.plans.coverage(&plan).have, 0, "configs, indexes and manifest read from the CDN only");
    assert!(plan.bytes() > 0);
}

#[test]
fn coverage_counts_what_the_stash_holds() {
    let w = holding_x();
    let plan = w.plans.plan("osi", &version()).unwrap();
    w.settle();
    let c = w.plans.coverage(&plan);
    // Planning read the configs, the indexes and the manifest through the cache.
    let read: u64 = plan.files[..7].iter().map(|f| f.1).sum::<u64>() + (w.x_at - 10);
    assert_eq!((c.have, c.want), (read + X_SIZE, plan.bytes()));
    assert!(!c.full());
    // A client fetches L.
    w.get(&at("data", &key(L), ""), None);
    assert_eq!(w.plans.coverage(&plan).have, read + X_SIZE + L_SIZE);
    // Half of Z is not Z.
    w.get(&at("data", ARCHIVE_2, ""), Some("bytes=0-1499"));
    assert_eq!(w.plans.coverage(&plan).have, read + X_SIZE + L_SIZE);
    w.get(&at("data", ARCHIVE_2, ""), Some("bytes=1500-2999"));
    w.get(&at("data", &key(ENC), ""), None);
    assert!(w.plans.coverage(&plan).full());
}

#[test]
fn detail_splits_whole_files_from_archive_ranges() {
    let w = holding_x();
    let plan = w.plans.plan("osi", &version()).unwrap();
    w.settle();
    let d = w.plans.detail(&plan);
    let dl = w.x_at - 10;
    assert_eq!((d.file_count, d.part_count, d.archive_count, d.requests, d.absent), (9, 3, 2, 11, 2));
    assert_eq!((d.archived.have, d.archived.want), (dl + X_SIZE, dl + X_SIZE + Z_SIZE), "the manifest was read");
    assert_eq!(d.files.want - d.files.have, ENC_SIZE + L_SIZE, "the loose files are not");
    let c = w.plans.coverage(&plan);
    assert_eq!((c.have, c.want), (d.files.have + d.archived.have, d.files.want + d.archived.want));
}
