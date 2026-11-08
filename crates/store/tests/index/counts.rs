//! Tests for `lanstash_store::index::counts`.

use lanstash_domain::extent::Span;
use lanstash_domain::segment::footer::key_hash;
use lanstash_store::index::Index;
use lanstash_store::index::counts::Counts;

use super::{file, seg};

#[test]
fn counts_what_is_held_now_and_every_segment_registered() {
    let idx = Index::new();
    assert_eq!(idx.counts(), Counts::default());
    let a = seg(&idx, "7");
    let _b = seg(&idx, "8");
    idx.insert_record(key_hash(b"steam/a"), a, 0, 10, 5);
    let f = idx.add_file(file("blizzard/x", 10));
    idx.insert_file_extent(f, Span::new(0, 5));
    assert_eq!(idx.counts(), Counts { segments: 2, object_files: 1, objects: 2 });
    idx.remove_segment(a);
    idx.remove_file(f);
    assert_eq!(idx.counts(), Counts { segments: 1, object_files: 0, objects: 0 }, "evicted ones are not held");
    assert_eq!(idx.segment_count(), 2, "registered, evicted ones included");
}
