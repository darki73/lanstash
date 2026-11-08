//! Tests for `lanstash_store::game::load`.

use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::segment::scan::read_sealed_index;
use lanstash_store::test_support::MemFs;

use crate::{load, put};

#[test]
fn an_unsealed_segment_is_continued_after_a_crash() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    put(&game, &index, SegmentOptions::default(), "steam/a", &[1; 50]);
    drop((game, index)); // a crash: nothing sealed

    let (game, index, report) = load(&fs, false, 2);
    assert_eq!((report.segments_unsealed, report.records, report.continued), (1, 1, 1));
    assert!(matches!(index.lookup("steam/a"), Lookup::Found(_)));
    put(&game, &index, SegmentOptions::default(), "steam/b", &[2; 50]);
    assert_eq!(fs.paths().iter().filter(|p| p.ends_with(".lcseg")).count(), 1, "written on in place");
}

#[test]
fn read_only_loads_change_nothing() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    put(&game, &index, SegmentOptions::default(), "steam/a", &[1; 50]);
    drop((game, index));
    let before = fs.paths();

    let (_, index, report) = load(&fs, true, 2);
    assert_eq!((report.records, report.continued, report.sealed_at_start), (1, 0, 0));
    assert!(matches!(index.lookup("steam/a"), Lookup::Found(_)));
    assert_eq!(fs.paths(), before);
    assert!(read_sealed_index(&fs.file("steam/7/seg/00000.lcseg")).unwrap().is_none(), "still unsealed");
}
