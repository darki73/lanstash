//! Tests for `lanstash_store::layout`.

use lanstash_store::layout::*;

#[test]
fn paths() {
    let g = GameDir::new("blizzard", "hs");
    assert_eq!(g.path(), "blizzard/hs");
    assert_eq!(g.segment(7), "blizzard/hs/seg/00007.lcseg");
    assert_eq!(g.journal(), "blizzard/hs/journal.lsj");
    assert_eq!(g.file(&object_rel_path("tpr/hs/data/8e/fc/8efc")), "blizzard/hs/obj/tpr/hs/data/8e/fc/8efc");
    assert_eq!(GameDir::new("steam", "").game, "_");
    assert_eq!(GameDir::new("x", "../y").path(), "x/..%2Fy");
}

#[test]
fn segment_names() {
    assert_eq!(parse_segment_name("00042.lcseg"), Some(42));
    assert_eq!(parse_segment_name("123456.lcseg"), Some(123_456));
    for bad in ["steam-1-00000.lcseg", ".lcseg", "00001.lcseg.tmp", "x1.lcseg"] {
        assert_eq!(parse_segment_name(bad), None, "{bad}");
    }
}
