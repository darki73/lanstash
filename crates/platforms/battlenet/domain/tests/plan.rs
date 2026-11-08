//! Assembling a plan from where a build's files lie, and its figures.

use lanstash_battlenet_domain::locate::{Located, Place};
use lanstash_battlenet_domain::plan::{Build, Coverage, PlanDetail, assemble};
use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::extent::Span;

fn key(b: u8) -> String {
    format!("{b:02x}").repeat(16)
}

fn build() -> Build {
    Build {
        product: "osi".into(),
        version: Version { build_config: "b".repeat(32), cdn_config: "c".repeat(32), name: "3.3.7".into() },
        path: "tpr/osi".into(),
        hosts: vec!["eu.cdn".into(), "level3".into()],
        archives: vec!["/tpr/osi/data/a1/00/a1".into(), "/tpr/osi/data/a2/00/a2".into()],
        files: vec![("/tpr/osi/config/bb/bb/b".into(), 120)],
    }
}

fn at(key: String, place: Place) -> Located {
    Located { key, place }
}

#[test]
fn archive_files_become_sorted_parts_and_loose_files_whole_files() {
    let located = [
        at(key(1), Place::Archive { archive: 1, offset: 500, size: 100 }),
        at(key(2), Place::Archive { archive: 0, offset: 0, size: 10 }),
        at(key(3), Place::Loose { size: 40 }),
        at(key(3), Place::Loose { size: 40 }),
        at(key(4), Place::Absent),
    ];
    let plan = assemble(build(), &located, 64).unwrap();
    assert_eq!((plan.host.as_str(), plan.hosts.len()), ("eu.cdn", 2), "planning reads from the first host");
    assert_eq!(
        plan.parts,
        vec![(0, Span::new(0, 10), key(2)), (1, Span::new(500, 600), key(1))],
        "sorted by archive and offset"
    );
    let loose = format!("/tpr/osi/data/03/03/{}", key(3));
    assert_eq!(plan.files, vec![("/tpr/osi/config/bb/bb/b".into(), 120), (loose, 40)], "once each");
    assert_eq!(plan.absent, 1);
    assert_eq!(plan.requests.len(), 2, "one range per archive");
    assert_eq!(plan.bytes(), 120 + 40 + 10 + 100);
}

#[test]
fn a_loose_key_that_is_no_hash_is_an_error() {
    let located = [at("not a hash".into(), Place::Loose { size: 1 })];
    assert!(assemble(build(), &located, 64).is_err());
}

#[test]
fn coverage_is_full_only_when_everything_wanted_is_held() {
    assert!(Coverage { have: 5, want: 5 }.full());
    assert!(!Coverage { have: 4, want: 5 }.full());
    assert!(!Coverage::default().full(), "nothing wanted is not complete");
    let d = PlanDetail {
        files: Coverage { have: 1, want: 2 },
        archived: Coverage { have: 3, want: 4 },
        file_count: 2,
        part_count: 3,
        archive_count: 1,
        requests: 3,
        absent: 0,
    };
    assert_eq!(d.coverage(), Coverage { have: 4, want: 6 });
}
