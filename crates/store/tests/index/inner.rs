//! Tests for the index's inner state (`lanstash_store::index::inner`), through
//! the public `Index`: how pieces are added and dropped, and per-game usage.

use lanstash_domain::extent::Span;
use lanstash_domain::segment::footer::key_hash;
use lanstash_store::index::*;
use lanstash_store::layout::GameDir;

use crate::{file, seg};

#[test]
fn duplicates_keep_the_first_copy_and_overlaps_add_only_new_bytes() {
    let idx = Index::new();
    let s = seg(&idx, "1");
    idx.insert_record(key_hash(b"k"), s, 4096, 10, 100);
    idx.insert_record(key_hash(b"k"), s, 8192, 10, 100); // second copy of the same object
    let Lookup::Found(o) = idx.lookup("k") else { panic!() };
    assert!(matches!(&o.at(0).unwrap().1, Source::Record { offset: 4096, .. }));

    let f = idx.add_file(file("blizzard/o", 1000));
    idx.insert_file_extent(f, Span::new(100, 300));
    idx.insert_file_extent(f, Span::new(0, 200)); // overlaps: only 0..100 is new
    idx.insert_file_extent(f, Span::new(250, 400));
    let Lookup::Found(o) = idx.lookup("blizzard/o") else { panic!() };
    let spans: Vec<Span> = o.pieces.iter().map(|(s, _)| *s).collect();
    assert_eq!(spans, vec![Span::new(0, 400)]);
    assert_eq!(idx.stored_bytes(), 500);
}

#[test]
fn conflicts_make_an_object_unusable() {
    let idx = Index::new();
    let s = seg(&idx, "1");
    idx.insert_record(key_hash(b"k"), s, 4096, 10, 100);
    idx.insert_record(key_hash(b"k"), s, 8192, 10, 120); // same key, other length
    assert!(matches!(idx.lookup("k"), Lookup::Unusable));

    let f = idx.add_file(file("blizzard/z", 1000));
    idx.insert_file_extent(f, Span::new(0, 2000)); // past the total
    assert!(matches!(idx.lookup("blizzard/z"), Lookup::Unusable));

    // A whole record over part of an object file's extents.
    let f = idx.add_file(file("blizzard/w", 100));
    idx.insert_file_extent(f, Span::new(0, 10));
    idx.insert_record(key_hash(b"blizzard/w"), s, 12288, 10, 100);
    assert!(matches!(idx.lookup("blizzard/w"), Lookup::Unusable));

    // A file whose key differs from the requested one (same hash) is never served.
    let idx = Index::new();
    idx.add_file(file("blizzard/q", 10));
    idx.mark_unusable("blizzard/q");
    assert!(matches!(idx.lookup("blizzard/q"), Lookup::Unusable));
}

#[test]
fn removing_the_last_piece_forgets_the_object_and_its_conflict() {
    let idx = Index::new();
    let s = seg(&idx, "1");
    idx.insert_record(key_hash(b"k"), s, 4096, 10, 100);
    idx.insert_record(key_hash(b"k"), s, 8192, 10, 120);
    assert!(matches!(idx.lookup("k"), Lookup::Unusable));
    assert!(idx.remove_segment(s).is_some());
    assert!(matches!(idx.lookup("k"), Lookup::Missing), "fetched again after eviction");
    assert!(idx.remove_segment(s).is_none(), "ids are not reused");

    let f = idx.add_file(file("blizzard/x", 100));
    idx.insert_file_extent(f, Span::new(0, 50));
    assert_eq!(idx.file_bytes(f), 50);
    assert!(idx.remove_file(f).is_some());
    assert!(matches!(idx.lookup("blizzard/x"), Lookup::Missing));
    assert!(idx.is_empty());
}

#[test]
fn usage_counts_each_object_once_per_game() {
    let idx = Index::new();
    let a = seg(&idx, "a");
    let b = seg(&idx, "b");
    idx.insert_record(key_hash(b"x"), a, 0, 10, 100);
    idx.insert_record(key_hash(b"y"), a, 200, 10, 50);
    idx.insert_record(key_hash(b"z"), b, 0, 10, 7);
    let f = idx.add_file(file("blizzard/f", 100));
    idx.insert_file_extent(f, Span::new(0, 10));
    idx.insert_file_extent(f, Span::new(20, 30));
    idx.touch_file(f, 5);
    let u = idx.usage();
    assert_eq!(u[&GameDir::new("steam", "a")], Usage { objects: 2, bytes: 150, last_used_ms: 0 });
    assert_eq!(u[&GameDir::new("steam", "b")], Usage { objects: 1, bytes: 7, last_used_ms: 0 });
    assert_eq!(u[&GameDir::new("blizzard", "hs")], Usage { objects: 1, bytes: 20, last_used_ms: 5 });
    assert_eq!(idx.file_used(f), 5);
}
