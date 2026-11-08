//! Tests for `lanstash_store::compact`.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_store::evict::EvictItem;
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::{Lookup, Source};
use lanstash_store::layout::GameDir;
use lanstash_store::store::*;
use lanstash_store::test_support::{FileOp, ManualClock, MemFs};
use std::sync::Arc;

fn open(fs: &MemFs, clock: &Arc<ManualClock>) -> Store {
    // Small segments: three 20 KB records fill one.
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 64 << 10, alignment: 4096 }, read_only: false };
    Store::open(Roots::single(Arc::new(fs.clone())), opts, Arc::clone(clock) as _).unwrap().0
}

fn put(s: &Store, key: &str, byte: u8) {
    let b = vec![byte; 20_000];
    let w = s.reserve_record(ObjectRef { service: "steam", game: "7", key }, b"meta", 20_000).unwrap();
    w.write(0, &b).unwrap();
    w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
}

/// The stored body of `key`, if it is found.
fn body(s: &Store, key: &str) -> Option<Vec<u8>> {
    let Lookup::Found(o) = s.lookup_quietly(key) else { return None };
    let (span, Source::Record { reader, offset, header_len, .. }) = &o.pieces[0] else { return None };
    let mut b = vec![0u8; span.len() as usize];
    reader.read_exact_at(offset + u64::from(*header_len), &mut b).unwrap();
    Some(b)
}

fn dir() -> GameDir {
    GameDir::new("steam", "7")
}

#[test]
fn copies_the_wanted_objects_out_and_deletes_the_segment() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    put(&s, "steam/7/old1", 1);
    put(&s, "steam/7/live", 2);
    put(&s, "steam/7/old2", 3); // segment 0 is full
    put(&s, "steam/7/next", 4); // segment 1, the active one
    let before = s.stored_bytes();

    let c = s.compact_segment(&dir(), 0, &|k| k == "steam/7/live").unwrap();
    assert_eq!((c.records, c.copied_bytes), (1, 20_000));
    assert!(c.segment_bytes > 60_000, "{c:?}");
    assert_eq!(c.freed_bytes(), c.segment_bytes - 20_000);

    assert!(fs.get("steam/7/seg/00000.lcseg").is_none(), "the segment went");
    assert_eq!(body(&s, "steam/7/live"), Some(vec![2u8; 20_000]), "the copy is served");
    assert!(body(&s, "steam/7/old1").is_none() && body(&s, "steam/7/old2").is_none());
    assert_eq!(body(&s, "steam/7/next"), Some(vec![4u8; 20_000]));
    assert_eq!(s.stored_bytes(), before - 40_000);
    let segs: Vec<EvictItem> = s.eviction_candidates().into_iter().map(|v| v.item).collect();
    assert!(!segs.contains(&EvictItem::Segment(0)), "{segs:?}");
}

#[test]
fn the_copies_are_synced_before_the_segment_goes() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    put(&s, "steam/7/old", 1);
    put(&s, "steam/7/live", 2);
    put(&s, "steam/7/old2", 3);
    put(&s, "steam/7/next", 4);
    let active = fs.get("steam/7/seg/00001.lcseg").unwrap();
    let before = active.ops().len();
    s.compact_segment(&dir(), 0, &|k| k == "steam/7/live").unwrap();
    let ops = active.ops();
    let last_write = ops.iter().rposition(|o| matches!(o, FileOp::Write { .. })).unwrap();
    assert!(last_write >= before, "the copy went to the active segment");
    assert!(ops[last_write..].contains(&FileOp::Sync), "{:?}", &ops[before..]);
}

#[test]
fn the_copies_and_their_stored_headers_survive_a_restart() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    {
        let s = open(&fs, &clock);
        for (i, k) in ["a", "b", "c"].iter().enumerate() {
            put(&s, &format!("steam/7/{k}"), i as u8 + 1);
        }
        put(&s, "steam/7/d", 9);
        s.compact_segment(&dir(), 0, &|k| k != "steam/7/b").unwrap();
        for a in s.close_segments() {
            let _ = a.appender.seal_when_idle();
        }
    }
    let s = open(&fs, &clock);
    assert_eq!(body(&s, "steam/7/a"), Some(vec![1u8; 20_000]));
    assert_eq!(body(&s, "steam/7/c"), Some(vec![3u8; 20_000]));
    assert!(body(&s, "steam/7/b").is_none());
    let Lookup::Found(o) = s.lookup_quietly("steam/7/a") else { panic!("a") };
    let Source::Record { reader, offset, header_len, .. } = &o.pieces[0].1 else { panic!("record") };
    let mut h = vec![0u8; *header_len as usize];
    reader.read_exact_at(*offset, &mut h).unwrap();
    let h = lanstash_domain::segment::record::RecordHeader::decode(&h).unwrap();
    assert_eq!((h.key.as_str(), h.meta.as_slice()), ("steam/7/a", b"meta".as_slice()));
}

#[test]
fn refuses_the_segment_being_written_and_unknown_ones() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    put(&s, "steam/7/a", 1);
    assert!(s.compact_segment(&dir(), 0, &|_| true).is_err(), "active");
    assert!(s.compact_segment(&dir(), 5, &|_| true).is_err(), "unknown");
    assert!(s.compact_segment(&GameDir::new("steam", "8"), 0, &|_| true).is_err(), "unknown game");
    assert_eq!(body(&s, "steam/7/a"), Some(vec![1u8; 20_000]));
}

#[test]
fn a_second_copy_that_is_not_served_is_not_copied_again() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    put(&s, "steam/7/a", 1);
    put(&s, "steam/7/a", 1); // written again: stored, not served
    put(&s, "steam/7/b", 2);
    put(&s, "steam/7/c", 3);
    let c = s.compact_segment(&dir(), 0, &|_| true).unwrap();
    assert_eq!(c.records, 2, "{c:?}");
    assert_eq!(body(&s, "steam/7/a"), Some(vec![1u8; 20_000]));
    assert_eq!(body(&s, "steam/7/b"), Some(vec![2u8; 20_000]));
}
