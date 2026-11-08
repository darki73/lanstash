//! Tests for `lanstash_adapter_logs::rotating_file`.

use lanstash_adapter_logs::rotating_file::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lanstash-test-rotate-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn read(p: &Path, suffix: &str) -> Option<String> {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    fs::read_to_string(PathBuf::from(s)).ok()
}

#[test]
fn rotates_by_size_and_keeps_the_newest() {
    let d = dir("keep");
    let log = d.join("access.log");
    fs::write(&log, "old\n").unwrap();
    let mut f = RotatingFile::open(&log, Rotation { max_bytes: 10, keep: 2 }).unwrap();
    for line in ["aaaa\n", "bbbb\n", "cccc\n", "dddd\n", "eeee\n"] {
        f.write_all(line.as_bytes()).unwrap();
    }
    // At most 10 bytes per file, whole lines; two old files kept.
    assert_eq!(read(&log, "").as_deref(), Some("dddd\neeee\n"));
    assert_eq!(read(&log, ".1").as_deref(), Some("bbbb\ncccc\n"));
    assert_eq!(read(&log, ".2").as_deref(), Some("old\naaaa\n"));
    assert_eq!(read(&log, ".3"), None);
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn never_rotates_with_zero_and_starts_over_without_keep() {
    let d = dir("zero");
    let log = d.join("e.log");
    let mut f = RotatingFile::open(&log, Rotation { max_bytes: 0, keep: 2 }).unwrap();
    f.write_all(&[b'x'; 100]).unwrap();
    assert_eq!(fs::metadata(&log).unwrap().len(), 100);
    drop(f);
    let mut f = RotatingFile::open(&log, Rotation { max_bytes: 50, keep: 0 }).unwrap();
    f.write_all(b"new\n").unwrap();
    assert_eq!(read(&log, "").as_deref(), Some("new\n"));
    assert_eq!(read(&log, ".1"), None);
    let _ = fs::remove_dir_all(&d);
}
