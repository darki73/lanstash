//! The in-memory doubles behave like the file system they stand in for.

mod nginx;

use lanstash_domain::evict::Disk;
use lanstash_ports::clock::Clock;
use lanstash_ports::storage::{DirEntry, FileRead, FileWrite, StoreErrorKind, StoreFs};
use lanstash_store::test_support::{ManualClock, MemFile, MemFs};

#[test]
fn mem_file_reads_writes_and_fails_on_request() {
    let f = MemFile::default();
    f.write_all_at(2, b"ab").unwrap();
    assert_eq!(f.bytes(), b"\0\0ab");
    let mut buf = [0u8; 4];
    assert_eq!(f.read_exact_at(1, &mut buf).unwrap_err().kind, StoreErrorKind::UnexpectedEof);
    f.fail_nth_write(2);
    f.write_all_at(0, b"x").unwrap();
    assert!(f.write_all_at(0, b"y").is_err());
    f.write_all_at(0, b"z").unwrap();
    f.fail_writes(true);
    assert!(f.write_all_at(0, b"w").is_err());
    f.sync().unwrap();
    assert_eq!(f.syncs(), 1);
}

#[test]
fn mem_fs_lists_creates_renames_and_reports_space() {
    let fs = MemFs::default();
    fs.create_new("steam/7/a").unwrap().write_all_at(0, b"1").unwrap();
    assert_eq!(fs.create_new("steam/7/a").err().unwrap().kind, StoreErrorKind::AlreadyExists);
    fs.rename("steam/7/a", "steam/7/b").unwrap();
    assert_eq!(fs.list("steam").unwrap(), vec![DirEntry { name: "7".into(), is_dir: true }]);
    assert_eq!(fs.list("steam/7").unwrap(), vec![DirEntry { name: "b".into(), is_dir: false }]);
    assert_eq!(fs.open_read("steam/7/b").unwrap().size().unwrap(), 1);
    assert_eq!(fs.open_read("steam/7/a").err().unwrap().kind, StoreErrorKind::NotFound);
    assert_eq!(fs.paths(), vec!["steam/7/b"]);
    assert_eq!(fs.space(), Ok(None));
    fs.set_space(Some(Disk { free: 1, total: 2 }));
    assert_eq!(fs.space(), Ok(Some(Disk { free: 1, total: 2 })));
    fs.remove_file("steam/7/b").unwrap();
    assert!(fs.get("steam/7/b").is_none());
}

#[test]
fn manual_clock_moves_when_told() {
    let c = ManualClock::default();
    c.set(42);
    assert_eq!(c.now_ms(), 42);
}
