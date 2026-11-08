//! Tests for `lanstash_store::index`.

use lanstash_domain::extent::Span;
use lanstash_domain::segment::footer::key_hash;
use lanstash_store::game::ObjectFile;
use lanstash_store::index::*;
use lanstash_store::layout::GameDir;
use lanstash_store::test_support::MemFile;
use std::sync::Arc;

mod counts;
mod inner;
mod records;

fn seg(index: &Index, game: &str) -> SegId {
    index.add_segment(SegInfo {
        path: format!("steam/{game}/seg/00000.lcseg"),
        game: GameDir::new("steam", game),
        reader: Arc::new(MemFile::default()),
    })
}

fn file(key: &str, total: u64) -> Arc<ObjectFile> {
    Arc::new(ObjectFile::for_tests(GameDir::new("blizzard", "hs"), key, total))
}

#[test]
fn whole_records_and_lookups() {
    let idx = Index::new();
    let s = seg(&idx, "7");
    idx.insert_record(key_hash(b"steam/a"), s, 4096, 100, 5000);
    let Lookup::Found(o) = idx.lookup("steam/a") else { panic!() };
    assert_eq!((o.total, o.pieces.len()), (5000, 1));
    assert!(o.covers(Span::new(0, 5000)) && !o.covers(Span::new(0, 5001)));
    assert!(matches!(&o.at(10).unwrap().1, Source::Record { offset: 4096, header_len: 100, .. }));
    assert!(o.meta.is_none());
    assert!(matches!(idx.lookup("steam/b"), Lookup::Missing));
    assert_eq!(idx.usage()[&GameDir::new("steam", "7")], Usage { objects: 1, bytes: 5000, last_used_ms: 0 });
    idx.touch_segment(s, 77);
    assert!(matches!(idx.lookup_touch("steam/a", 50), Lookup::Found(_)));
    assert_eq!(idx.usage()[&GameDir::new("steam", "7")].last_used_ms, 77, "the latest use of any of its segments");
    assert!(matches!(idx.lookup_touch("steam/a", 90), Lookup::Found(_)));
    assert_eq!(idx.usage()[&GameDir::new("steam", "7")].last_used_ms, 90);
    assert!(matches!(idx.lookup("steam/a"), Lookup::Found(_)));
    assert_eq!(idx.usage()[&GameDir::new("steam", "7")].last_used_ms, 90, "a plain lookup is not a use");
}

#[test]
fn counts_the_changes_of_what_each_service_holds() {
    let idx = Index::new();
    assert_eq!((idx.changes("steam"), idx.changes("blizzard")), (0, 0));
    let s = seg(&idx, "7");
    assert_eq!(idx.changes("steam"), 0, "an empty segment holds nothing new");
    idx.insert_record(key_hash(b"steam/a"), s, 0, 10, 5);
    idx.insert_record(key_hash(b"steam/b"), s, 64, 10, 5);
    assert_eq!(idx.changes("steam"), 2);
    let f = idx.add_file(file("blizzard/x", 10));
    idx.insert_file_extent(f, Span::new(0, 5));
    assert_eq!((idx.changes("steam"), idx.changes("blizzard")), (2, 1), "one service's change is not another's");
    idx.remove_file(f);
    idx.remove_segment(s);
    assert_eq!((idx.changes("steam"), idx.changes("blizzard")), (3, 2));
    assert_eq!(idx.all_changes(), 5);
    let _ = idx.lookup_touch("steam/a", 9);
    assert_eq!(idx.changes("steam"), 3, "a use is not a change");
}

#[test]
fn a_segment_evicted_or_replaced_is_a_change_of_its_service() {
    // Eviction, a game's deletion, prune and compaction all forget a segment this way; what
    // was worked out from the stash (e.g. whether a game is current) depends on the count.
    let idx = Index::new();
    let s = seg(&idx, "7");
    idx.insert_record(key_hash(b"steam/a"), s, 0, 10, 5);
    let before = idx.changes("steam");
    assert!(idx.replace_segment(s, &[]).is_some());
    assert_eq!(idx.changes("steam"), before + 1);
    assert!(matches!(idx.lookup("steam/a"), Lookup::Missing));
    assert!(idx.replace_segment(s, &[]).is_none());
    assert_eq!(idx.changes("steam"), before + 1, "a segment already gone is no change");
}

#[test]
fn an_object_found_unusable_is_a_change_of_its_service() {
    // What a lookup answers for it changes: whatever was worked out from the stash is stale.
    let idx = Index::new();
    let s = seg(&idx, "7");
    idx.insert_record(key_hash(b"steam/a"), s, 0, 10, 5);
    let before = idx.changes("steam");
    idx.mark_unusable("steam/a");
    assert_eq!(idx.changes("steam"), before + 1);
    assert!(matches!(idx.lookup("steam/a"), Lookup::Unusable));
    idx.mark_unusable("steam/a");
    idx.mark_unusable("steam/nothing");
    assert_eq!(idx.changes("steam"), before + 1, "already unusable, or not stored: no change");
}

#[test]
fn a_second_file_of_a_key_making_it_unusable_is_a_change() {
    let idx = Index::new();
    idx.add_file(file("blizzard/x", 10));
    let before = idx.changes("blizzard");
    idx.add_file(file("blizzard/x", 20));
    assert!(matches!(idx.lookup("blizzard/x"), Lookup::Unusable));
    assert_eq!(idx.changes("blizzard"), before + 1);
}

#[test]
fn file_extents_join_and_know_the_total() {
    let idx = Index::new();
    let f = idx.add_file(file("blizzard/x", 1000));
    let Lookup::Found(o) = idx.lookup("blizzard/x") else { panic!() };
    assert_eq!((o.total, o.pieces.len()), (1000, 0), "total known before any extent");
    idx.insert_file_extent(f, Span::new(100, 200));
    idx.insert_file_extent(f, Span::new(0, 100));
    idx.insert_file_extent(f, Span::new(500, 600));
    let Lookup::Found(o) = idx.lookup("blizzard/x") else { panic!() };
    let spans: Vec<Span> = o.pieces.iter().map(|(s, _)| *s).collect();
    assert_eq!(spans, vec![Span::new(0, 200), Span::new(500, 600)]);
    assert!(o.covers(Span::new(50, 150)) && !o.covers(Span::new(150, 550)));
    assert!(o.meta.is_some());
    assert_eq!(idx.stored_bytes(), 300);
    assert!(idx.file_of("blizzard/x").is_some() && idx.file_of("blizzard/y").is_none());
}

#[test]
fn next_record_follows_offsets() {
    let idx = Index::new();
    let s = seg(&idx, "1");
    idx.insert_record(key_hash(b"b"), s, 8192, 10, 100);
    idx.insert_record(key_hash(b"a"), s, 4096, 10, 100);
    assert_eq!(idx.next_record(s, 4096).map(|r| r.0), Some(8192));
    assert!(idx.next_record(s, 8192).is_none());
    assert!(idx.next_record(s, 5000).is_none());
}
