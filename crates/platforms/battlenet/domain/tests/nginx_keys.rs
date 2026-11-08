//! A plan's pieces as lancache's nginx slice keys, and what present slices cover.

use lanstash_battlenet_domain::locate::Located;
use lanstash_battlenet_domain::nginx_keys::{Tally, tally, wanted};
use lanstash_battlenet_domain::plan::{Build, Plan, assemble};
use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::extent::Span;

const MIB: u64 = 1 << 20;

fn plan(files: Vec<(String, u64)>, parts: Vec<(usize, Span, String)>) -> Plan {
    let build = Build {
        product: "osi".into(),
        version: Version { build_config: "b".repeat(32), cdn_config: "c".repeat(32), name: "1".into() },
        path: "tpr/osi".into(),
        hosts: vec!["cdn".into()],
        archives: vec!["/tpr/osi/data/a1/00/a1".into()],
        files,
    };
    let mut p = assemble(build, &[] as &[Located], 64).unwrap();
    p.parts = parts;
    p
}

#[test]
fn a_range_needs_the_slices_it_touches() {
    let p = plan(
        vec![("/tpr/osi/data/aa/bb/big".into(), 3 * MIB), ("/tpr/osi/config/cc".into(), 0)],
        vec![(0, Span::new(MIB - 10, MIB + 10), "x".into())],
    );
    let want = wanted(&p, "blizzard", MIB);
    let names = |ks: &[usize]| ks.iter().map(|&k| want.keys[k].clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&want.whole[0].1),
        vec![
            "blizzard/tpr/osi/data/aa/bb/bigbytes=0-1048575",
            "blizzard/tpr/osi/data/aa/bb/bigbytes=1048576-2097151",
            "blizzard/tpr/osi/data/aa/bb/bigbytes=2097152-3145727",
        ]
    );
    assert_eq!(names(&want.whole[1].1), vec!["blizzard/tpr/osi/config/ccbytes=0-1048575"], "unknown size: one slice");
    let a = &p.archives[0];
    assert_eq!(
        names(&want.archived[0].1),
        vec![format!("blizzard{a}bytes=0-1048575"), format!("blizzard{a}bytes=1048576-2097151")]
    );
}

#[test]
fn shared_slices_are_keyed_once_and_tallied_per_piece() {
    let parts = vec![(0, Span::new(0, 10), "x".into()), (0, Span::new(20, 30), "y".into())];
    let p = plan(Vec::new(), parts);
    let want = wanted(&p, "blizzard", MIB);
    assert_eq!(want.keys.len(), 1, "both parts lie in the archive's first slice");
    assert_eq!(want.archived, vec![(10, vec![0]), (10, vec![0])]);
    assert_eq!(tally(&want.archived, &[true]), Tally { wanted: 2, found: 2, bytes: 20, bytes_found: 20 });
    assert_eq!(tally(&want.archived, &[false]), Tally { wanted: 2, found: 0, bytes: 20, bytes_found: 0 });
}
