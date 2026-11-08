//! Tests for `lanstash_store::verify`.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::extent::Span;
use lanstash_domain::segment::header::HEADER_REGION;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::storage::FileWrite;
use lanstash_store::store::*;
use lanstash_store::test_support::MemFs;
use lanstash_store::verify::{ItemKind, ItemResult, plan};
use std::sync::Arc;

fn open(fs: &MemFs) -> Store {
    Store::open(Roots::single(Arc::new(fs.clone())), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap().0
}

fn body(n: usize, seed: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

/// Two records in a segment, an object file with two extents, and a record
/// reserved but never committed (a download still running).
fn sample(fs: &MemFs) -> Store {
    let s = open(fs);
    for (key, b) in [("steam/a", body(5000, 1)), ("steam/b", body(300, 2))] {
        let w = s.reserve_record(ObjectRef { service: "steam", game: "7", key }, b"", b.len() as u64).unwrap();
        w.write(0, &b).unwrap();
        w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
    }
    let running = s.reserve_record(ObjectRef { service: "steam", game: "7", key: "steam/c" }, b"", 100).unwrap();
    std::mem::forget(running);

    let obj = ObjectRef { service: "blizzard", game: "hs", key: "blizzard/tpr/hs/x" };
    let f = s.object_file(obj, "/tpr/hs/x", 100_000, b"").unwrap();
    let data = body(100_000, 3);
    for span in [Span::new(0, 4096), Span::new(50_000, 100_000)] {
        let part = &data[span.start as usize..span.end as usize];
        f.writer().unwrap().write_all_at(span.start, part).unwrap();
        s.commit_extent(&f, span, crc32c(part)).unwrap();
    }
    s
}

fn check_all(fs: &MemFs) -> (Vec<ItemResult>, Vec<(String, String)>) {
    let p = plan(&Roots::single(Arc::new(fs.clone()))).unwrap();
    let mut results: Vec<ItemResult> = p.items.iter().map(|i| i.check()).collect();
    results.sort_by(|a, b| a.path.cmp(&b.path));
    (results, p.problems)
}

#[test]
fn a_healthy_store_checks_clean() {
    let fs = MemFs::default();
    let _s = sample(&fs);
    let (r, problems) = check_all(&fs);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(r.len(), 2);
    let (file, seg) = (&r[0], &r[1]);
    assert_eq!((file.kind, file.ok, file.ok_bytes), (ItemKind::ObjectFile, 2, 54_096));
    assert_eq!((seg.kind, seg.ok, seg.ok_bytes), (ItemKind::Segment, 2, 5300));
    assert!(file.problems.is_empty() && seg.problems.is_empty(), "{r:?}");
}

#[test]
fn a_record_still_being_written_is_not_damage() {
    let fs = MemFs::default();
    let s = sample(&fs);
    // Its body written, its header not yet (it is written when the record is committed).
    let w = s.reserve_record(ObjectRef { service: "steam", game: "7", key: "steam/d" }, b"", 10_000).unwrap();
    w.write(0, &body(10_000, 4)).unwrap();
    let after = s.reserve_record(ObjectRef { service: "steam", game: "7", key: "steam/e" }, b"", 10).unwrap();
    after.write(0, &body(10, 5)).unwrap();
    after.commit(RecordFlags::default(), crc32c(&body(10, 5))).unwrap();
    let (r, _) = check_all(&fs);
    let seg = &r[1];
    assert_eq!(seg.ok, 3, "{seg:?}");
    assert!(seg.problems.is_empty(), "{seg:?}");
    drop(w);
}

#[test]
fn damaged_bytes_are_found_in_records_and_extents() {
    let fs = MemFs::default();
    let _s = sample(&fs);
    let seg = fs.get("steam/7/seg/00000.lcseg").unwrap();
    // A byte in the middle of the first record's body.
    seg.with_bytes(|b| b[HEADER_REGION as usize + 2000] ^= 0xff);
    let obj = fs.paths().into_iter().find(|p| p.contains("/obj/")).unwrap();
    fs.get(&obj).unwrap().with_bytes(|b| b[60_000] ^= 1);

    let (r, _) = check_all(&fs);
    let (file, seg) = (&r[0], &r[1]);
    assert_eq!((file.ok, file.problems.len()), (1, 1), "{file:?}");
    assert!(file.problems[0].starts_with("bytes 50000..100000: checksum"), "{file:?}");
    assert_eq!((seg.ok, seg.problems.len()), (1, 1), "{seg:?}");
    assert!(seg.problems[0].contains("body checksum mismatch") && seg.problems[0].contains("steam/a"), "{seg:?}");
}

#[test]
fn a_missing_object_file_and_a_torn_journal_are_reported() {
    let fs = MemFs::default();
    let _s = sample(&fs);
    let obj = fs.paths().into_iter().find(|p| p.contains("/obj/")).unwrap();
    fs.remove(&obj);
    fs.get("blizzard/hs/journal.lsj").unwrap().with_bytes(|b| b.extend_from_slice(b"LJ\x03"));

    let (r, problems) = check_all(&fs);
    assert!(r[0].problems[0].starts_with("can't open"), "{r:?}");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].0, "blizzard/hs/journal.lsj");
    assert!(problems[0].1.starts_with("3 bytes after the last valid entry"), "{problems:?}");
}
