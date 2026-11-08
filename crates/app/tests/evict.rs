//! Tests for `lanstash_app::evict`.

use lanstash_app::evict::{EvictOptions, Evictor};
use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::evict::{Disk, KeepFree, Limits};
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::events::{Event, Events};
use lanstash_store::evict::EvictItem;
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::store::{ObjectRef, Roots, Store, StoreOptions};
use lanstash_store::test_support::{ManualClock, MemFs};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Log(Mutex<Vec<String>>);

impl Events for Log {
    fn emit(&self, e: &Event<'_>) {
        if let Event::Evicted { path, .. } = e {
            self.0.lock().unwrap().push((*path).to_owned());
        }
    }
}

const MIN: u64 = 60_000;

/// Three games written at minutes 1, 2, 3; each fills one sealed segment
/// (3 × 20 KB) and starts a second, active one.
fn setup() -> (MemFs, Arc<ManualClock>, Arc<Store>) {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: 64 << 10, alignment: 4096 }, read_only: false };
    let s = Store::open(Roots::single(Arc::new(fs.clone())), opts, Arc::clone(&clock) as _).unwrap().0;
    for (n, game) in ["1", "2", "3"].iter().enumerate() {
        clock.set((n as u64 + 1) * MIN);
        for i in 0..4 {
            let key = format!("steam/{game}/{i}");
            let b = vec![1u8; 20_000];
            let w = s.reserve_record(ObjectRef { service: "steam", game, key: &key }, b"", 20_000).unwrap();
            w.write(0, &b).unwrap();
            w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
        }
    }
    (fs, clock, Arc::new(s))
}

fn evictor(s: &Arc<Store>, clock: &Arc<ManualClock>, limits: Limits, log: &Arc<Log>) -> Evictor {
    let opts = EvictOptions { limits, protect_ms: 10 * MIN, cooldown_ms: 2 * MIN };
    Evictor::new(Arc::clone(s), Arc::clone(log) as _, Arc::clone(clock) as _, opts)
}

fn found(s: &Store, key: &str) -> bool {
    matches!(s.lookup(key), Lookup::Found(_))
}

#[test]
fn nothing_happens_without_limits_or_below_them() {
    let (_fs, clock, s) = setup();
    let log = Arc::new(Log::default());
    clock.set(100 * MIN);
    assert!(evictor(&s, &clock, Limits::default(), &log).run_once().skipped);
    let e = evictor(&s, &clock, Limits { max_size: Some(1 << 30), keep_free: None }, &log);
    let p = e.run_once();
    assert!(!p.skipped && p.evicted.is_empty() && p.over_max_size == 0);
    assert_eq!(e.counters.stored_bytes.load(std::sync::atomic::Ordering::Relaxed), 240_000);
}

#[test]
fn the_bytes_stored_follow_every_change_of_the_stash() {
    // Added up once per change (a walk of every object), they still follow each one.
    let (_fs, clock, s) = setup();
    let log = Arc::new(Log::default());
    let e = evictor(&s, &clock, Limits::default(), &log);
    let stored = || e.counters.stored_bytes.load(std::sync::atomic::Ordering::Relaxed);
    e.run_once();
    e.run_once();
    assert_eq!(stored(), 240_000);
    let b = vec![2u8; 10_000];
    let w = s.reserve_record(ObjectRef { service: "steam", game: "4", key: "steam/4/0" }, b"", 10_000).unwrap();
    w.write(0, &b).unwrap();
    w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
    e.run_once();
    assert_eq!(stored(), 250_000, "a new record");
}

#[test]
fn max_size_evicts_least_recently_used_segments_first() {
    let (_fs, clock, s) = setup();
    let log = Arc::new(Log::default());
    clock.set(100 * MIN);
    assert!(found(&s, "steam/1/0"), "game 1 used just now: game 2's segment is the oldest");
    // 240 KB stored; at most 180 KB: one sealed segment (60 KB) must go.
    let e = evictor(&s, &clock, Limits { max_size: Some(180_000), keep_free: None }, &log);
    let p = e.run_once();
    assert_eq!(p.over_max_size, 60_000);
    assert_eq!(p.evicted.len(), 1, "{p:?}");
    assert_eq!(*log.0.lock().unwrap(), vec!["steam/2/seg/00000.lcseg"]);
    assert!(!found(&s, "steam/2/0") && found(&s, "steam/2/3") && found(&s, "steam/3/0"));
    assert_eq!(e.counters.evicted_segments.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Cooling down, then satisfied.
    assert!(e.run_once().skipped);
    clock.set(103 * MIN);
    let p = e.run_once();
    assert!(!p.skipped && p.evicted.is_empty());
}

#[test]
fn recently_used_items_are_protected_even_when_short() {
    let (_fs, clock, s) = setup();
    let log = Arc::new(Log::default());
    clock.set(12 * MIN); // games 2 and 3 (minutes 2, 3) are within the 10 minutes
    let e = evictor(&s, &clock, Limits { max_size: Some(0), keep_free: None }, &log);
    let p = e.run_once();
    let gone: Vec<_> = p.evicted.iter().map(|(v, _)| (v.game.game.clone(), v.item)).collect();
    assert_eq!(gone, vec![("1".to_owned(), EvictItem::Segment(0))]);
    assert!(e.counters.short_bytes.load(std::sync::atomic::Ordering::Relaxed) > 0);
}

#[test]
fn keep_free_evicts_until_the_disk_has_room() {
    let (fs, clock, s) = setup();
    let log = Arc::new(Log::default());
    clock.set(100 * MIN);
    fs.set_space(Some(Disk { free: 1_000_000, total: 10_000_000 }));
    let e = evictor(&s, &clock, Limits { max_size: None, keep_free: Some(KeepFree::Percent(11)) }, &log);
    let p = e.run_once();
    assert_eq!(p.short_of_free, vec![100_000]);
    let paths = log.0.lock().unwrap().clone();
    assert_eq!(paths, vec!["steam/1/seg/00000.lcseg", "steam/2/seg/00000.lcseg"], "oldest first, until 100 KB");
    assert_eq!(e.counters.disk_free_bytes.load(std::sync::atomic::Ordering::Relaxed), 1_000_000);
}
