//! Tests for `lanstash_domain::zip`, on an archive made by Python's `zipfile`
//! (one deflated entry `z`, one stored entry `plain.txt`) from text written here.

use lanstash_domain::zip::{ZipError, entries, first, read};

const ZIP: &str = "504b03041400000008000000655ded0c917c1b0000001c020000010000007acb49cc2b2e492cce50c84dcccb4c4b2d2e51c819151915018b0000504b03041400000000000000655dac24bbe00c0000000c00000009000000706c61696e2e74787473746f726564206173206973504b010214001400000008000000655ded0c917c1b0000001c0200000100000000000000000000008001000000007a504b010214001400000000000000655dac24bbe00c0000000c00000009000000000000000000000080013a000000706c61696e2e747874504b05060000000002000200660000006d0000000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn both_entries_read_back() {
    let zip = unhex(ZIP);
    let list = entries(&zip).unwrap();
    assert_eq!(list.iter().map(|e| (e.name.as_str(), e.method)).collect::<Vec<_>>(), [("z", 8), ("plain.txt", 0)]);
    assert_eq!(first(&zip, 1 << 20).unwrap(), b"lanstash manifest ".repeat(30));
    assert_eq!(read(&zip, &list[1], 1 << 20).unwrap(), b"stored as is");
}

#[test]
fn damage_and_limits_are_errors() {
    let zip = unhex(ZIP);
    assert_eq!(first(&zip, 100), Err(ZipError::TooLarge));
    let mut bad = zip.clone();
    let list = entries(&zip).unwrap();
    let data = list[1].offset as usize + 30 + 9; // the stored entry's first byte
    bad[data] ^= 0x20;
    assert_eq!(read(&bad, &list[1], 1 << 20), Err(ZipError::BadChecksum));
    assert_eq!(entries(b"not a zip at all, not even close"), Err(ZipError::NotZip));
}
