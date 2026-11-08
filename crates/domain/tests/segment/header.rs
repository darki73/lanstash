//! Tests for `lanstash_domain::segment::header`.

use lanstash_domain::segment::error::FormatError;
use lanstash_domain::segment::header::*;

fn sample() -> SegmentHeader {
    SegmentHeader {
        flags: 0,
        alignment: 4096,
        segment_id: 17,
        service: Service::STEAM,
        game_id: 2_357_570,
        created_at_ms: 1_793_822_984_000,
        max_bytes: 1 << 30,
    }
}

#[test]
fn round_trip() {
    let h = sample();
    let enc = h.encode().unwrap();
    assert_eq!(&enc[..8], b"LANSTASH");
    assert_eq!(SegmentHeader::decode(&enc).unwrap(), h);
}

#[test]
fn detects_corruption_in_any_byte() {
    let enc = sample().encode().unwrap();
    for i in 0..HEADER_ENCODED_LEN {
        let mut bad = enc;
        bad[i] ^= 0x01;
        assert!(SegmentHeader::decode(&bad).is_err(), "flip at byte {i} not detected");
    }
}

#[test]
fn rejects_bad_alignment() {
    for a in [0, 3, 8192] {
        let h = SegmentHeader { alignment: a, ..sample() };
        assert!(h.encode().is_err(), "alignment {a}");
    }
}

#[test]
fn rejects_short_and_future_version() {
    let enc = sample().encode().unwrap();
    assert!(matches!(SegmentHeader::decode(&enc[..10]), Err(FormatError::TooShort { .. })));
    let mut v2 = enc;
    v2[8..10].copy_from_slice(&2u16.to_le_bytes()); // version field
    assert_eq!(SegmentHeader::decode(&v2), Err(FormatError::UnsupportedVersion(2)));
}

#[test]
fn unknown_service_is_preserved() {
    let h = SegmentHeader { service: Service(999), ..sample() };
    let d = SegmentHeader::decode(&h.encode().unwrap()).unwrap();
    assert_eq!(d.service, Service(999));
    assert_eq!(d.service.name(), "unrecognised");
}
