//! `DirFs` against a real temporary directory.

mod support;

use std::fs;
use std::sync::Arc;

use lanstash_adapter_fs::dir_fs::DirFs;
use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::extent::Span;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::storage::{DirEntry, FileRead, FileWrite, StoreErrorKind, StoreFs};
use lanstash_store::index::Lookup;
use lanstash_store::store::{ObjectRef, Roots, Store, StoreOptions};
use support::TempDir;

#[test]
fn dir_fs_creates_lists_renames_and_rejects_escapes() {
    let d = TempDir::new("dirfs");
    let s = DirFs::open(&d.0.join("root")).unwrap();
    s.create_new("steam/7/seg/00000.lcseg").unwrap().write_all_at(0, b"x").unwrap();
    s.create_new("steam/7/journal.lsj.tmp").unwrap();
    s.rename("steam/7/journal.lsj.tmp", "steam/7/journal.lsj").unwrap();
    let names: Vec<DirEntry> = s.list("steam/7").unwrap();
    assert_eq!(
        names,
        vec![DirEntry { name: "journal.lsj".into(), is_dir: false }, DirEntry { name: "seg".into(), is_dir: true }]
    );
    assert_eq!(s.list("").unwrap(), vec![DirEntry { name: "steam".into(), is_dir: true }]);
    assert_eq!(s.list("nope").unwrap_err().kind, StoreErrorKind::NotFound);
    assert_eq!(s.open_read("steam/7/seg/00000.lcseg").unwrap().size().unwrap(), 1);
    s.remove_file("steam/7/journal.lsj").unwrap();
    for bad in ["../x", "a/../../x", "/abs", "a//b", "a/./b", "c:x", "a\\b", "a/"] {
        assert!(s.create_new(bad).is_err(), "{bad} must be rejected");
        assert!(s.open_read(bad).is_err(), "{bad} must be rejected");
    }
}

#[test]
fn immutable_opens_rotate_across_views() {
    let d = TempDir::new("views");
    fs::create_dir_all(d.0.join("a")).unwrap();
    fs::create_dir_all(d.0.join("b")).unwrap();
    // Same name in both "views"; different sizes reveal which path served the open.
    fs::write(d.0.join("a").join("x"), b"1").unwrap();
    fs::write(d.0.join("b").join("x"), b"22").unwrap();
    let s = DirFs::with_read_paths(&d.0.join("a"), &[d.0.join("b")]).unwrap();
    assert_eq!(s.path_count(), 2);
    let sizes: Vec<u64> = (0..4).map(|_| s.open_read_immutable("x").unwrap().size().unwrap()).collect();
    assert_eq!(sizes, vec![1, 2, 1, 2]);
    assert_eq!(s.open_read("x").unwrap().size().unwrap(), 1, "other reads use the primary");
    s.create_new("y").unwrap();
    assert!(d.0.join("a").join("y").exists() && !d.0.join("b").join("y").exists());

    fs::write(d.0.join("a").join("z"), b"").unwrap();
    let err = DirFs::with_read_paths(&d.0.join("a"), &[d.0.join("b")]).unwrap_err();
    assert!(err.message.contains("not the same directory"), "{}", err.message);
    assert_eq!(
        DirFs::with_read_paths(&d.0.join("a"), &[d.0.join("missing")]).unwrap_err().kind,
        StoreErrorKind::NotFound
    );
}

#[test]
fn store_on_disk_survives_reopen() {
    let d = TempDir::new("store");
    let open = || {
        let fs: Arc<dyn StoreFs> = Arc::new(DirFs::open(&d.0).unwrap());
        Store::open(Roots::single(fs), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap().0
    };
    let store = open();
    let rec = ObjectRef { service: "steam", game: "7", key: "steam/depot/7/chunk/aa" };
    let w = store.reserve_record(rec, b"content-type=x\n", 3).unwrap();
    w.write(0, b"abc").unwrap();
    w.commit(RecordFlags::default(), crc32c(b"abc")).unwrap();
    let obj = ObjectRef { service: "blizzard", game: "hs", key: "blizzard/tpr/hs/data/8e/fc/8efc" };
    let file = store.object_file(obj, "/tpr/hs/data/8e/fc/8efc", 1 << 30, b"").unwrap();
    file.writer().unwrap().write_all_at(1 << 29, b"middle").unwrap();
    store.commit_extent(&file, Span::at(1 << 29, 6), crc32c(b"middle")).unwrap();
    store.close_segments();
    drop((store, file));

    let store = open();
    assert!(matches!(store.lookup(rec.key), Lookup::Found(o) if o.covers(Span::new(0, 3))));
    let Lookup::Found(o) = store.lookup(obj.key) else { panic!("object file lost") };
    assert_eq!(o.total, 1 << 30);
    assert!(o.covers(Span::at(1 << 29, 6)) && !o.covers(Span::at(0, 1)));
    assert!(d.0.join("blizzard/hs/obj/tpr/hs/data/8e/fc/8efc").exists());
}
