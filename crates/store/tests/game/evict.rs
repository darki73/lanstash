//! Tests for `lanstash_store::game::evict`: last-use times saved and loaded
//! (eviction itself is tested through the store in `tests/evict.rs`).

use lanstash_store::game::SegmentOptions;
use lanstash_store::test_support::MemFs;

use crate::{dir, load, put};

#[test]
fn last_use_survives_a_restart_and_unknown_items_count_as_new() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    put(&game, &index, SegmentOptions::default(), "steam/a", &[1; 50]);
    let seg = game.close_active().unwrap().id;
    index.touch_segment(seg, 40);
    game.save_recency(&index).unwrap();
    drop((game, index));

    let (_, index, _) = load(&fs, false, 99);
    assert_eq!(index.segment_used(0), 40, "the saved time, not the load time");

    fs.remove(&dir().recency());
    let (_, index, _) = load(&fs, false, 99);
    assert_eq!(index.segment_used(0), 1, "without the file: the segment's creation time");
}
