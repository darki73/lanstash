//! `FsFile` against a real temporary directory.

mod support;

use lanstash_adapter_fs::fs_file::FsFile;
use lanstash_ports::storage::{FileRead, FileWrite, StoreErrorKind};
use support::TempDir;

#[test]
fn file_reads_writes_and_reports_eof() {
    let d = TempDir::new("file");
    let path = d.0.join("f");
    let f = FsFile::create_new(&path).unwrap();
    f.write_all_at(5, b"abc").unwrap();
    assert_eq!(f.size().unwrap(), 8);
    let mut buf = [9u8; 8];
    f.read_exact_at(0, &mut buf).unwrap();
    assert_eq!(&buf, b"\0\0\0\0\0abc");
    assert_eq!(f.read_exact_at(4, &mut buf).unwrap_err().kind, StoreErrorKind::UnexpectedEof);
    f.set_len(1 << 20).unwrap();
    f.sync().unwrap();
    assert_eq!(FsFile::open_read(&path).unwrap().size().unwrap(), 1 << 20);
    assert_eq!(FsFile::create_new(&path).unwrap_err().kind, StoreErrorKind::AlreadyExists);
    assert_eq!(FsFile::open_read(&d.0.join("missing")).unwrap_err().kind, StoreErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_not_followed() {
    let d = TempDir::new("link");
    let target = d.0.join("elsewhere");
    std::fs::write(&target, b"not the stash's").unwrap();
    let link = d.0.join("f");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(FsFile::open_write(&link).is_err());
    assert!(FsFile::open_read(&link).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"not the stash's");
}

#[test]
fn the_os_file_under_it_is_offered() {
    let d = TempDir::new("os");
    let f = FsFile::create_new(&d.0.join("f")).unwrap();
    assert!(f.os_file().is_some_and(|a| a.downcast_ref::<std::fs::File>().is_some()), "a std::fs::File");
}
