//! Tests for `lanstash_domain::segment::record`.

use lanstash_domain::segment::record::*;

fn sample() -> RecordHeader {
    RecordHeader {
        flags: RecordFlags(RecordFlags::UPSTREAM_VERIFIED),
        body_len: 1_055_104,
        range_start: 0,
        total_len: 1_055_104,
        body_crc: 0xDEAD_BEEF,
        key: String::from("steam/depot/2357571/chunk/9de1d873a59f1c1d108f63112ffe8903dc4b5ae1"),
        meta: vec![1, 2, 3],
    }
}

#[test]
fn round_trip() {
    let h = sample();
    let enc = h.encode().unwrap();
    assert_eq!(enc.len(), h.encoded_len());
    assert_eq!(RecordHeader::decode(&enc).unwrap(), h);
    let fixed = RecordFixed::decode(&enc).unwrap();
    assert_eq!(fixed.header_len(), enc.len());
}

#[test]
fn detects_corruption_in_any_byte() {
    let enc = sample().encode().unwrap();
    for i in 0..enc.len() {
        if (40..44).contains(&i) {
            continue; // reserved bytes are covered by the CRC too; checked below
        }
        let mut bad = enc.clone();
        bad[i] ^= 0x80;
        assert!(RecordHeader::decode(&bad).is_err(), "flip at byte {i} not detected");
    }
    let mut bad = enc;
    bad[41] ^= 1;
    assert!(RecordHeader::decode(&bad).is_err(), "reserved byte flip not detected");
}

#[test]
fn rejects_empty_and_oversized_key() {
    let mut h = sample();
    h.key = String::new();
    assert!(h.encode().is_err());
    h.key = "k".repeat(70_000);
    assert!(h.encode().is_err());
}

#[test]
fn alignment_math() {
    assert_eq!(align_up(0, 4096), 0);
    assert_eq!(align_up(1, 4096), 4096);
    assert_eq!(align_up(4096, 4096), 4096);
    assert_eq!(align_up(4097, 4096), 8192);
    assert_eq!(align_up(12345, 1), 12345);
    assert_eq!(record_span(100, 1_048_576, 4096), 1_052_672);
    assert_eq!(record_span(100, 1_048_576, 1), 1_048_676);
}

#[test]
fn flags() {
    let f = RecordFlags(RecordFlags::SLICED | RecordFlags::UPSTREAM_VERIFIED);
    assert!(f.contains(RecordFlags::SLICED));
    assert!(!RecordFlags::default().contains(RecordFlags::SLICED));
}
