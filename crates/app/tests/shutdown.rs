//! Tests for `lanstash_app::shutdown`.

use lanstash_app::shutdown::Shutdown;
use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_store::segment::scan::read_sealed_index;
use lanstash_store::store::{ObjectRef, Roots, Store, StoreOptions};
use lanstash_store::test_support::{ManualClock, MemFs};
use std::sync::Arc;

#[test]
fn seals_idle_segments_waits_for_busy_ones_and_saves_last_use() {
    let fs = MemFs::default();
    let clock = Arc::new(ManualClock::default());
    let s = Arc::new(Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), clock as _).unwrap().0);
    let put = |game: &str| {
        let b = vec![1u8; 100];
        let key = format!("steam/{game}/a");
        let w = s.reserve_record(ObjectRef { service: "steam", game, key: &key }, b"", 100).unwrap();
        w.write(0, &b).unwrap();
        w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
    };
    put("1");
    put("2");
    // A download still writing into game 2's segment.
    let busy = s.reserve_record(ObjectRef { service: "steam", game: "2", key: "steam/2/b" }, b"", 100).unwrap();

    let stop = Shutdown::begin(Arc::clone(&s));
    assert_eq!(stop.open(), 1);
    let sealed = |p: &str| read_sealed_index(&fs.get(p).unwrap()).unwrap().is_some();
    assert!(sealed("steam/1/seg/00000.lcseg") && !sealed("steam/2/seg/00000.lcseg"));
    busy.write(0, &[2u8; 100]).unwrap();
    busy.commit(RecordFlags::default(), crc32c(&[2u8; 100])).unwrap();
    assert_eq!(stop.open(), 0, "sealed as soon as the last record is done");
    assert!(sealed("steam/2/seg/00000.lcseg"));

    let r = stop.finish();
    assert_eq!((r.segments_sealed, r.segments_open, r.recency_failed.len()), (2, 0, 0));
    assert!(fs.get("steam/1/used.lsu").is_some() && fs.get("steam/2/used.lsu").is_some());
}
