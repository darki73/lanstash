//! Tests for `lanstash_store::store`.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::extent::Span;
use lanstash_domain::journal::{Entry, read_journal};
use lanstash_domain::segment::header::HEADER_REGION;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::storage::StoreFs;
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::index::Source;
use lanstash_store::layout::GameDir;
use lanstash_store::segment::scan::{ScanOptions, read_sealed_index, scan};
use lanstash_store::store::*;
use lanstash_store::test_support::MemFs;
use std::collections::HashMap;
use std::sync::Arc;

fn open(fs: &MemFs, seg_max: u64) -> (Store, LoadReport) {
    let opts = StoreOptions { segments: SegmentOptions { max_bytes: seg_max, alignment: 4096 }, read_only: false };
    Store::open(Roots::single(Arc::new(fs.clone())), opts, Arc::new(FixedClock(1))).unwrap()
}

fn body(n: usize, seed: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

fn put_record(s: &Store, service: &str, game: &str, key: &str, b: &[u8]) {
    let w = s.reserve_record(ObjectRef { service, game, key }, b"content-type=x\n", b.len() as u64).unwrap();
    for (i, part) in b.chunks(1000).enumerate() {
        w.write((i * 1000) as u64, part).unwrap();
    }
    w.commit(RecordFlags::default(), crc32c(b)).unwrap();
}

fn read_record(s: &Store, key: &str) -> Vec<u8> {
    let Lookup::Found(o) = s.lookup(key) else { panic!("{key} not found") };
    let (_, Source::Record { reader, offset, header_len, .. }) = &o.pieces[0] else { panic!() };
    let mut b = vec![0u8; o.total as usize];
    reader.read_exact_at(offset + u64::from(*header_len), &mut b).unwrap();
    b
}

#[test]
fn records_go_to_per_game_segments_and_survive_a_restart() {
    let fs = MemFs::default();
    let (s, rep) = open(&fs, 64 << 20);
    assert_eq!(rep.games, 0);
    put_record(&s, "steam", "7", "steam/depot/7/chunk/a", &body(5000, 1));
    put_record(&s, "steam", "7", "steam/depot/7/chunk/b", &body(300, 2));
    put_record(&s, "steam", "8", "steam/depot/8/chunk/c", &body(10, 3));
    assert_eq!(read_record(&s, "steam/depot/7/chunk/b"), body(300, 2));
    assert_eq!(fs.paths(), vec!["steam/7/seg/00000.lcseg", "steam/8/seg/00000.lcseg"]);
    drop(s); // a crash: nothing sealed

    let (s, rep) = open(&fs, 64 << 20);
    assert_eq!((rep.games, rep.totals.records, rep.totals.segments_unsealed, rep.totals.continued), (2, 3, 2, 2));
    assert_eq!(read_record(&s, "steam/depot/7/chunk/a"), body(5000, 1));
    put_record(&s, "steam", "7", "steam/depot/7/chunk/d", &body(20, 4));
    assert_eq!(fs.paths().len(), 2, "the unsealed segment was continued");
    let u = s.usage()[&GameDir::new("steam", "7")];
    assert_eq!((u.objects, u.bytes), (3, 5320));
    assert!(u.last_used_ms > 0, "writes count as use");
}

#[test]
fn holds_says_whether_a_game_has_anything_stored_and_after_a_restart_too() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, 64 << 20);
    assert!(!s.holds("blizzard", "auks"));
    put_record(&s, "blizzard", "osi", "blizzard/tpr/osi/config/ab/cd/x", &body(10, 1));
    assert!(s.holds("blizzard", "osi") && !s.holds("blizzard", "auks") && !s.holds("steam", "osi"));
    drop(s);
    let (s, _) = open(&fs, 64 << 20);
    assert!(s.holds("blizzard", "osi"), "reloaded");
    // Its one segment gone (a deletion): it holds nothing.
    let Lookup::Found(o) = s.lookup("blizzard/tpr/osi/config/ab/cd/x") else { panic!() };
    let (_, Source::Record { seg, .. }) = &o.pieces[0] else { panic!() };
    s.index().remove_segment(*seg);
    assert!(!s.holds("blizzard", "osi"));
}

#[test]
fn full_segments_rotate_and_seal() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, HEADER_REGION + 8192);
    for i in 0..5 {
        put_record(&s, "steam", "1", &format!("steam/depot/1/chunk/{i}"), &body(100, i));
    }
    assert_eq!(fs.paths().len(), 3);
    for p in &fs.paths()[..2] {
        assert!(read_sealed_index(&fs.file(p)).unwrap().is_some(), "{p} sealed");
    }
    s.close_segments();
    let (s, rep) = open(&fs, HEADER_REGION + 8192);
    assert_eq!((rep.totals.segments_sealed, rep.totals.records), (3, 5));
    assert_eq!(read_record(&s, "steam/depot/1/chunk/4"), body(100, 4));
}

#[test]
fn an_abandoned_record_leaves_no_entry() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, 64 << 20);
    let w = s.reserve_record(ObjectRef { service: "steam", game: "1", key: "k1" }, b"", 10).unwrap();
    w.write(0, b"01234").unwrap();
    drop(w);
    put_record(&s, "steam", "1", "k2", b"later");
    assert!(matches!(s.lookup("k1"), Lookup::Missing));
    let rep = scan(&fs.file("steam/1/seg/00000.lcseg"), ScanOptions { verify_bodies: true }).unwrap();
    assert_eq!(rep.records.len(), 1);
    s.close_segments();
    assert!(read_sealed_index(&fs.file("steam/1/seg/00000.lcseg")).unwrap().is_some());
}

#[test]
fn object_files_are_sparse_journaled_and_reloaded() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, 64 << 20);
    let obj = ObjectRef { service: "blizzard", game: "hs", key: "blizzard/tpr/hs/data/8e/fc/8efc" };
    let f = s.object_file(obj, "/tpr/hs/data/8e/fc/8efc", 1_000_000, b"etag=x\n").unwrap();
    assert_eq!(f.path, "blizzard/hs/obj/tpr/hs/data/8e/fc/8efc");
    assert_eq!(fs.file(&f.path).bytes().len(), 1_000_000, "created at full length");
    let data = body(4096, 9);
    f.writer().unwrap().write_all_at(500_000, &data).unwrap();
    s.commit_extent(&f, Span::at(500_000, 4096), crc32c(&data)).unwrap();
    assert!(fs.file(&f.path).syncs() >= 1, "data synced before the journal entry");
    let again = s.object_file(obj, "/tpr/hs/data/8e/fc/8efc", 1_000_000, b"").unwrap();
    assert!(Arc::ptr_eq(&f, &again));
    assert!(s.object_file(obj, "/tpr/hs/data/8e/fc/8efc", 5, b"").is_err(), "conflicting total");

    let Lookup::Found(o) = s.lookup(obj.key) else { panic!() };
    assert_eq!((o.total, o.pieces.len(), o.meta.as_deref()), (1_000_000, 1, Some(&b"etag=x\n"[..])));
    drop(s);

    let (s, rep) = open(&fs, 64 << 20);
    assert_eq!((rep.totals.object_files, rep.totals.extents, rep.totals.extent_bytes), (1, 1, 4096));
    let Lookup::Found(o) = s.lookup(obj.key) else { panic!() };
    assert_eq!(o.pieces[0].0, Span::at(500_000, 4096));
    let Source::File { file } = &o.pieces[0].1 else { panic!() };
    let mut back = vec![0u8; 4096];
    file.reader().unwrap().read_exact_at(500_000, &mut back).unwrap();
    assert_eq!(back, data);
    let entries = read_journal(&fs.file("blizzard/hs/journal.lsj").bytes()).entries;
    assert!(matches!(entries[1], Entry::Object { total_len: 1_000_000, .. }));
}

#[test]
fn crash_leftovers_of_object_files_are_ignored_and_reused() {
    let fs = MemFs::default();
    // A file without OBJECT entry (crash after creating it).
    fs.file("blizzard/hs/obj/tpr/hs/x").with_bytes(|b| b.extend_from_slice(b"stale data"));
    let (s, _) = open(&fs, 64 << 20);
    assert!(matches!(s.lookup("blizzard/tpr/hs/x"), Lookup::Missing));
    let f = s
        .object_file(ObjectRef { service: "blizzard", game: "hs", key: "blizzard/tpr/hs/x" }, "/tpr/hs/x", 20, b"")
        .unwrap();
    assert_eq!(fs.file(&f.path).bytes(), vec![0u8; 20], "stale bytes are gone");
    // Data written but no EXTENT (crash before the journal entry): still missing after restart.
    f.writer().unwrap().write_all_at(0, b"0123456789").unwrap();
    drop((s, f));
    let (s, _) = open(&fs, 64 << 20);
    let Lookup::Found(o) = s.lookup("blizzard/tpr/hs/x") else { panic!() };
    assert!(o.pieces.is_empty());
}

#[test]
fn case_collisions_get_a_fallback_name() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, 64 << 20);
    let a = s.object_file(ObjectRef { service: "g", game: "_", key: "g/A" }, "/A", 5, b"").unwrap();
    let b = s.object_file(ObjectRef { service: "g", game: "_", key: "g/a" }, "/a", 5, b"").unwrap();
    assert_eq!(a.path, "g/_/obj/A");
    assert!(b.path.starts_with("g/_/obj/_h_"), "{}", b.path);
}

#[test]
fn per_service_roots() {
    let (main, fast) = (MemFs::default(), MemFs::default());
    let roots = Roots {
        default: Arc::new(main.clone()),
        per_service: HashMap::from([("steam".to_owned(), Arc::new(fast.clone()) as Arc<dyn StoreFs>)]),
    };
    let (s, _) = Store::open(roots.clone(), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    put_record(&s, "steam", "1", "steam/k", b"steam");
    put_record(&s, "blizzard", "hs", "blizzard/k", b"bliz");
    assert_eq!(fast.paths(), vec!["steam/1/seg/00000.lcseg"]);
    assert_eq!(main.paths(), vec!["blizzard/hs/seg/00000.lcseg"]);
    // A stray steam directory in the default root is not loaded twice.
    main.file("steam/9/seg/00000.lcseg");
    let (s, rep) = Store::open(roots, StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    assert_eq!(rep.ignored, vec!["steam"]);
    assert_eq!(read_record(&s, "steam/k"), b"steam");
}

#[test]
fn read_only_open_sees_everything_and_writes_nothing() {
    let fs = MemFs::default();
    let (s, _) = open(&fs, 1 << 20);
    put_record(&s, "steam", "7", "steam/depot/7/chunk/a", &body(5000, 1));
    let obj = ObjectRef { service: "blizzard", game: "hs", key: "blizzard/tpr/hs/data/x" };
    let f = s.object_file(obj, "/tpr/hs/data/x", 100_000, b"").unwrap();
    f.writer().unwrap().write_all_at(10, &body(50, 2)).unwrap();
    s.commit_extent(&f, Span::at(10, 50), crc32c(&body(50, 2))).unwrap();
    // The server is still "running": its segment stays open; a torn journal tail as well.
    fs.file("blizzard/hs/journal.lsj").with_bytes(|d| d.extend_from_slice(b"LJ\x02\x00"));
    let before: Vec<(String, Vec<u8>)> = fs.paths().into_iter().map(|p| (p.clone(), fs.file(&p).bytes())).collect();

    let opts = StoreOptions { read_only: true, ..StoreOptions::default() };
    let (ro, report) = Store::open(Roots::single(Arc::new(fs.clone())), opts, Arc::new(FixedClock(2))).unwrap();
    assert_eq!(read_record(&ro, "steam/depot/7/chunk/a"), body(5000, 1));
    assert!(matches!(ro.lookup(obj.key), Lookup::Found(o) if o.covers(Span::at(10, 50))));
    assert_eq!((report.totals.continued, report.totals.sealed_at_start), (0, 0));
    assert_eq!(report.totals.journal_truncated, 4, "the torn tail is counted, not cut");
    assert!(ro.reserve_record(obj, b"", 1).is_err() && ro.object_file(obj, "/x", 1, b"").is_err());
    assert!(ro.commit_extent(&f, Span::at(0, 1), 0).is_err());

    let after: Vec<(String, Vec<u8>)> = fs.paths().into_iter().map(|p| (p.clone(), fs.file(&p).bytes())).collect();
    assert_eq!(before, after, "a read-only open must not change any file");
}
