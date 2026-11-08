//! Tests for `lanstash_store::evict` (and the last-use times behind it).

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::extent::Span;
use lanstash_domain::journal::{Entry, read_journal};
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::storage::{FileWrite, StoreFs};
use lanstash_store::evict::{EvictItem, Victim};
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::layout::GameDir;
use lanstash_store::store::*;
use lanstash_store::test_support::{ManualClock, MemFs};
use std::sync::Arc;

fn open(fs: &MemFs, clock: &Arc<ManualClock>) -> Store {
    // Small segments: a few records each, so a game has several.
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 64 << 10, alignment: 4096 }, read_only: false };
    Store::open(Roots::single(Arc::new(fs.clone())), opts, Arc::clone(clock) as _).unwrap().0
}

fn put(s: &Store, game: &str, key: &str, len: usize) {
    let b = vec![7u8; len];
    let w = s.reserve_record(ObjectRef { service: "steam", game, key }, b"", len as u64).unwrap();
    w.write(0, &b).unwrap();
    w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
}

fn put_file(s: &Store, key: &str) {
    let f =
        s.object_file(ObjectRef { service: "blizzard", game: "hs", key }, &format!("/{key}"), 100_000, b"").unwrap();
    let data = vec![3u8; 50_000];
    f.writer().unwrap().write_all_at(0, &data).unwrap();
    s.commit_extent(&f, Span::new(0, 50_000), crc32c(&data)).unwrap();
}

fn found(s: &Store, key: &str) -> bool {
    matches!(s.lookup(key), Lookup::Found(_))
}

fn victim(s: &Store, item: EvictItem) -> Victim {
    s.eviction_candidates().into_iter().find(|v| v.item == item).unwrap()
}

#[test]
fn candidates_are_sealed_segments_and_object_files_with_their_last_use() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    clock.set(1000);
    let s = open(&fs, &clock);
    for i in 0..3 {
        put(&s, "7", &format!("steam/7/a{i}"), 20_000); // fills segment 0
    }
    clock.set(2000);
    put(&s, "7", "steam/7/b", 20_000); // goes to segment 1, now the active one
    clock.set(3000);
    put_file(&s, "x");

    let mut c = s.eviction_candidates();
    c.sort_by_key(|v| v.item);
    let items: Vec<(EvictItem, u64)> = c.iter().map(|v| (v.item, v.last_used_ms)).collect();
    assert_eq!(items, vec![(EvictItem::Segment(0), 1000), (EvictItem::File(0), 3000)], "{c:?}");
    assert_eq!(c[1].bytes, 50_000);
    assert!(c[0].bytes > 60_000, "segment 0 holds three 20 KB records: {}", c[0].bytes);

    clock.set(5000);
    assert!(found(&s, "steam/7/a1"));
    assert_eq!(victim(&s, EvictItem::Segment(0)).last_used_ms, 5000, "a hit is a use");
}

#[test]
fn evicting_a_segment_forgets_its_records_and_deletes_the_file() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    for i in 0..4 {
        put(&s, "7", &format!("steam/7/k{i}"), 20_000);
    }
    assert!(fs.get("steam/7/seg/00000.lcseg").is_some());
    let freed = s.evict(&victim(&s, EvictItem::Segment(0))).unwrap();
    assert!(freed > 60_000);
    assert!(fs.get("steam/7/seg/00000.lcseg").is_none());
    assert!(!found(&s, "steam/7/k0") && !found(&s, "steam/7/k2"));
    assert!(found(&s, "steam/7/k3"), "the active segment stays");
    // The active segment is never a candidate, and evicting it is refused.
    assert!(s.eviction_candidates().is_empty());
    let active = Victim {
        game: GameDir::new("steam", "7"),
        item: EvictItem::Segment(1),
        path: String::new(),
        bytes: 0,
        last_used_ms: 0,
        root: 0,
    };
    assert!(s.evict(&active).is_err());
    // Fetched again later: stored again.
    put(&s, "7", "steam/7/k0", 20_000);
    assert!(found(&s, "steam/7/k0"));

    // A restart agrees.
    drop(s);
    let s = open(&fs, &clock);
    assert!(!found(&s, "steam/7/k1") && found(&s, "steam/7/k0") && found(&s, "steam/7/k3"));
}

#[test]
fn evicting_an_object_file_is_journaled_and_survives_a_restart() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = open(&fs, &clock);
    put_file(&s, "x");
    put_file(&s, "y");
    let path = victim(&s, EvictItem::File(0)).path;
    assert_eq!(s.evict(&victim(&s, EvictItem::File(0))).unwrap(), 50_000);
    assert!(fs.get(&path).is_none());
    assert!(!found(&s, "x") && found(&s, "y"));
    let journal = read_journal(&fs.get("blizzard/hs/journal.lsj").unwrap().bytes());
    assert!(journal.entries.contains(&Entry::Forget { no: 0 }));

    drop(s);
    let s = open(&fs, &clock);
    assert!(!found(&s, "x") && found(&s, "y"));
    // The same object can be stored again, at the same path.
    put_file(&s, "x");
    assert!(found(&s, "x"));
    assert!(fs.get(&path).is_some());
}

#[test]
fn last_use_times_survive_a_restart() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    clock.set(1000);
    let s = open(&fs, &clock);
    for i in 0..4 {
        put(&s, "7", &format!("steam/7/k{i}"), 20_000);
    }
    put_file(&s, "x");
    clock.set(7000);
    assert!(found(&s, "x"));
    assert!(s.save_recency().is_empty());
    assert!(fs.get("blizzard/hs/used.lsu").is_some() && fs.get("blizzard/hs/used.lsu.tmp").is_none());
    put_file(&s, "y"); // never saved: counts as used at the restart
    drop(s);

    clock.set(9000);
    let s = open(&fs, &clock);
    let used = |item| victim(&s, item).last_used_ms;
    assert_eq!(used(EvictItem::Segment(0)), 1000);
    assert_eq!(used(EvictItem::File(0)), 7000);
    assert_eq!(used(EvictItem::File(1)), 9000);
}

#[test]
fn space_comes_from_the_root() {
    let fs = MemFs::default();
    assert_eq!(fs.space().unwrap(), None);
    fs.set_space(Some(lanstash_domain::evict::Disk { free: 5, total: 10 }));
    let s = open(&fs, &Arc::new(ManualClock::default()));
    assert_eq!(s.roots().len(), 1);
    assert_eq!(s.roots()[0].space().unwrap().map(|d| d.free), Some(5));
}

#[test]
fn a_store_without_saved_times_starts_from_segment_creation() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    clock.set(1000);
    let s = open(&fs, &clock);
    for i in 0..3 {
        put(&s, "7", &format!("steam/7/k{i}"), 20_000); // segment 0, created at 1000
    }
    clock.set(5000);
    put(&s, "7", "steam/7/k3", 20_000); // segment 1, created at 5000
    clock.set(6000);
    put_file(&s, "x"); // blizzard/hs: no segments at all
    drop(s); // nothing saved

    clock.set(9000);
    let s = open(&fs, &clock);
    assert_eq!(victim(&s, EvictItem::Segment(0)).last_used_ms, 1000);
    assert_eq!(victim(&s, EvictItem::File(0)).last_used_ms, 9000, "a game without segments: unknown, so now");
}

fn journal_entries(fs: &MemFs) -> usize {
    read_journal(&fs.get("blizzard/hs/journal.lsj").unwrap().bytes()).entries.len()
}

/// 700 object files with one extent each; the first 600 evicted.
fn mostly_evicted(fs: &MemFs, clock: &Arc<ManualClock>) -> Store {
    let s = open(fs, clock);
    for i in 0..700 {
        put_file(&s, &format!("f{i}"));
    }
    for v in s.eviction_candidates().into_iter().filter(|v| matches!(v.item, EvictItem::File(n) if n < 600)) {
        s.evict(&v).unwrap();
    }
    assert_eq!(journal_entries(fs), 1 + 700 * 2 + 600);
    s
}

#[test]
fn journals_of_evicted_files_are_compacted() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = mostly_evicted(&fs, &clock);
    let (done, failed) = s.compact_journals();
    assert_eq!((done, failed.len()), (vec![GameDir::new("blizzard", "hs")], 0));
    assert_eq!(journal_entries(&fs), 1 + 100 * 2);
    assert!(fs.get("blizzard/hs/journal.lsj.tmp").is_none());
    assert!(found(&s, "f650") && !found(&s, "f10"));
    // Still appendable, and the numbers did not change.
    put_file(&s, "new");
    assert!(s.compact_journals().0.is_empty(), "nothing dead left");
    drop(s);
    let (s, rep) = Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), clock as _).unwrap();
    assert_eq!((rep.totals.object_files, rep.totals.journal_bad, rep.totals.journals_compacted), (101, 0, 0));
    assert!(found(&s, "f699") && found(&s, "new") && !found(&s, "f0"));
    assert_eq!(victim(&s, EvictItem::File(650)).bytes, 50_000);
}

#[test]
fn a_mostly_dead_journal_is_compacted_at_startup() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    drop(mostly_evicted(&fs, &clock));
    let (s, rep) = Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), clock as _).unwrap();
    assert_eq!((rep.totals.journals_compacted, rep.totals.object_files), (1, 100));
    assert_eq!(journal_entries(&fs), 201);
    assert!(found(&s, "f600"));
}
