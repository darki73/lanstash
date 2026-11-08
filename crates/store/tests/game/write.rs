//! Tests for `lanstash_store::game::write`.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::extent::Span;
use lanstash_domain::object::fallback_rel_path;
use lanstash_domain::segment::header::HEADER_REGION;
use lanstash_ports::storage::StoreFs;
use lanstash_store::game::SegmentOptions;
use lanstash_store::index::Lookup;
use lanstash_store::test_support::MemFs;

use crate::{load, put};

#[test]
fn records_go_to_the_active_segment_and_a_full_one_rotates() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    let opts = SegmentOptions { max_bytes: HEADER_REGION + 8192, alignment: 4096 };
    for i in 0..3 {
        put(&game, &index, opts, &format!("steam/depot/7/chunk/{i}"), &[i; 100]);
    }
    assert!(matches!(index.lookup("steam/depot/7/chunk/2"), Lookup::Found(_)));
    let segs: Vec<String> = fs.paths().into_iter().filter(|p| p.ends_with(".lcseg")).collect();
    assert_eq!(segs, vec!["steam/7/seg/00000.lcseg", "steam/7/seg/00001.lcseg"]);
    let active = game.close_active().unwrap();
    assert_eq!(active.path, "steam/7/seg/00001.lcseg");
    assert!(active.appender.is_sealed(), "closing an idle segment seals it");
}

#[test]
fn object_files_keep_their_extents_in_the_journal() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    let f = game.object_file(&index, "blizzard/data/ab/cd/abcd", "/data/ab/cd/abcd", 1000, b"k=v\n").unwrap();
    assert!(std::ptr::eq(&*f, &*game.object_file(&index, "blizzard/data/ab/cd/abcd", "x", 1000, b"").unwrap()));
    f.writer().unwrap().write_all_at(0, &[7; 100]).unwrap();
    game.commit_extent(&index, &f, Span::new(0, 100), 0).unwrap();
    assert!(game.commit_extent(&index, &f, Span::new(900, 1001), 0).is_err(), "past the total");
    assert_eq!(index.file_bytes(f.id()), 100);
    assert!(!game.compact_journal().unwrap(), "too few dead entries to rewrite");
    drop((game, index));

    let (_, index, report) = load(&fs, false, 2);
    assert_eq!((report.object_files, report.extents, report.extent_bytes), (1, 1, 100));
    let Lookup::Found(o) = index.lookup("blizzard/data/ab/cd/abcd") else { panic!() };
    assert_eq!((o.total, &*o.meta.unwrap()), (1000, &b"k=v\n"[..]));
}

#[test]
fn an_object_never_takes_the_file_of_another() {
    let fs = MemFs::default();
    let (game, index, _) = load(&fs, false, 1);
    // B lives at the name the second key's hash would give.
    let second = "steam/a//b";
    let taken = format!("/{}", fallback_rel_path(second));
    let b = game.object_file(&index, &format!("steam{taken}"), &taken, 4, b"").unwrap();
    b.writer().unwrap().write_all_at(0, b"BBBB").unwrap();
    game.commit_extent(&index, &b, Span::new(0, 4), crc32c(b"BBBB")).unwrap();
    // Two keys with one safe path: the second falls back to its hash name, which B has.
    let first = game.object_file(&index, "steam/a/b", "/a/b", 4, b"").unwrap();
    let a = game.object_file(&index, second, "/a//b", 4, b"").unwrap();
    let paths = [&first.path, &a.path, &b.path];
    assert!(paths[0] != paths[1] && paths[1] != paths[2] && paths[0] != paths[2], "{paths:?}");
    a.writer().unwrap().write_all_at(0, b"AAAA").unwrap();
    let mut got = [0u8; 4];
    b.reader().unwrap().read_exact_at(0, &mut got).unwrap();
    assert_eq!(&got, b"BBBB", "B's bytes stay");
}

#[test]
fn a_file_left_over_from_a_crash_is_replaced_not_written_through() {
    let fs = MemFs::default();
    fs.create_new("steam/7/obj/data/x").unwrap().write_all_at(0, b"old bytes").unwrap();
    let (game, index, _) = load(&fs, false, 1);
    let f = game.object_file(&index, "steam/data/x", "/data/x", 4, b"").unwrap();
    assert_eq!(f.path, "steam/7/obj/data/x");
    let mut got = [9u8; 4];
    f.reader().unwrap().read_exact_at(0, &mut got).unwrap();
    assert_eq!(got, [0; 4], "a new, empty file of the object's length");
}
