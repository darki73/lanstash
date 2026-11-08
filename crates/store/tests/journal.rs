//! Tests for `lanstash_store::journal`.

use lanstash_domain::journal::{Entry, JOURNAL_VERSION};
use lanstash_store::journal::*;
use lanstash_store::test_support::MemFs;

fn extent(no: u32) -> Entry {
    Entry::Extent { no, start: 0, len: 10, crc: 1 }
}

#[test]
fn create_append_reopen() {
    let fs = MemFs::default();
    let mut j = JournalFile::create(&fs, "g/journal.lsj").unwrap();
    j.append(&extent(1)).unwrap();
    j.append(&extent(2)).unwrap();
    let (mut j2, opened) = JournalFile::open(&fs, "g/journal.lsj").unwrap();
    assert_eq!(opened.read.entries.len(), 3);
    assert_eq!((opened.truncated, j2.len()), (0, j.len()));
    j2.append(&extent(3)).unwrap();
    let (_, again) = JournalFile::open(&fs, "g/journal.lsj").unwrap();
    assert_eq!(again.read.entries.last(), Some(&extent(3)));
    assert!(JournalFile::create(&fs, "g/journal.lsj").is_err(), "create never clobbers");
}

#[test]
fn torn_tail_is_cut_and_overwritten() {
    let fs = MemFs::default();
    let mut j = JournalFile::create(&fs, "j").unwrap();
    j.append(&extent(1)).unwrap();
    let good = j.len();
    fs.file("j").with_bytes(|b| b.extend_from_slice(b"LJ\x03\x00\x18")); // half an entry
    let (mut j, opened) = JournalFile::open(&fs, "j").unwrap();
    assert_eq!((opened.truncated, j.len()), (5, good));
    j.append(&extent(2)).unwrap();
    let (_, again) = JournalFile::open(&fs, "j").unwrap();
    assert_eq!(again.read.entries.len(), 3);
    assert_eq!(again.read.stopped, None);
}

#[test]
fn missing_header_restarts_the_journal() {
    let fs = MemFs::default();
    fs.file("j").with_bytes(|b| b.extend_from_slice(b"garbage"));
    let (j, opened) = JournalFile::open(&fs, "j").unwrap();
    assert_eq!(opened.read.entries.len(), 0);
    assert!(!j.is_empty(), "a fresh header was written");
    let (_, created) = JournalFile::open_or_create(&fs, "new").unwrap();
    assert!(created.is_none());
}

#[test]
fn a_journal_of_a_newer_version_is_refused_and_left_as_it_is() {
    let fs = MemFs::default();
    let mut newer = Entry::Header { version: JOURNAL_VERSION + 1 }.encode().unwrap();
    newer.extend(extent(1).encode().unwrap());
    fs.file("j").with_bytes(|b| b.extend_from_slice(&newer));
    assert!(JournalFile::open(&fs, "j").is_err());
    assert!(JournalFile::open_or_create(&fs, "j").is_err());
    assert_eq!(fs.file("j").bytes(), newer, "untouched");
}

#[test]
fn a_damaged_entry_in_the_middle_keeps_the_ones_after_it() {
    let fs = MemFs::default();
    let mut j = JournalFile::create(&fs, "j").unwrap();
    for no in 1..=3 {
        j.append(&extent(no)).unwrap();
    }
    let len = j.len();
    drop(j);
    // A bit flipped in the second extent.
    let at = Entry::Header { version: JOURNAL_VERSION }.encode().unwrap().len() + extent(1).encode().unwrap().len() + 9;
    fs.file("j").with_bytes(|b| b[at] ^= 1);
    let (j, opened) = JournalFile::open(&fs, "j").unwrap();
    assert_eq!(opened.read.entries, [Entry::Header { version: JOURNAL_VERSION }, extent(1), extent(3)]);
    assert_eq!((opened.truncated, j.len()), (0, len), "nothing cut");
}

#[test]
fn failed_append_leaves_no_partial_entry() {
    let fs = MemFs::default();
    let mut j = JournalFile::create(&fs, "j").unwrap();
    let before = j.len();
    fs.file("j").fail_writes(true);
    assert!(j.append(&extent(1)).is_err());
    fs.file("j").fail_writes(false);
    assert_eq!(j.len(), before);
    j.append(&extent(2)).unwrap();
    let (_, o) = JournalFile::open(&fs, "j").unwrap();
    assert_eq!(o.read.entries, vec![Entry::Header { version: JOURNAL_VERSION }, extent(2)]);
}
