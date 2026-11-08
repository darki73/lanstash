//! Tests for `lanstash_store::segment::scan`.

use lanstash_domain::segment::footer::find_entries;
use lanstash_domain::segment::footer::key_hash;
use lanstash_store::segment::scan::*;
use lanstash_store::segment::writer::SegmentAppender;
use lanstash_store::test_support::{MemFile, rec, sample_header};

fn build(align: u32, n: usize) -> (MemFile, SegmentAppender<MemFile>, Vec<u64>) {
    let f = MemFile::default();
    let a = SegmentAppender::create(f.clone(), sample_header(align, 64 << 20)).unwrap();
    let offs = (0..n)
        .map(|i| a.append(rec(&format!("steam/depot/1/chunk/{i:040}")), &vec![i as u8; 3000 + i * 7]).unwrap().offset)
        .collect();
    (f, a, offs)
}

#[test]
fn sealed_segment_scans_clean_and_index_matches() {
    for align in [4096, 1] {
        let (f, a, _) = build(align, 20);
        a.seal().unwrap();
        let rep = scan(&f, ScanOptions { verify_bodies: true }).unwrap();
        assert!(rep.is_clean(), "align {align}: {:?}", rep.problems);
        assert_eq!(rep.records.len(), 20);
        assert!(rep.records.iter().all(|r| r.body_ok == Some(true)));
        let idx = read_sealed_index(&f).unwrap().unwrap();
        let key = "steam/depot/1/chunk/0000000000000000000000000000000000000007";
        let hit = find_entries(&idx.entries, key_hash(key.as_bytes()));
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].body_len, 3000 + 7 * 7);
    }
}

#[test]
fn unsealed_segment_is_recovered() {
    let (f, _a, offs) = build(4096, 5);
    assert!(read_sealed_index(&f).unwrap().is_none());
    let rep = scan(&f, ScanOptions::default()).unwrap();
    assert!(rep.sealed.is_none());
    assert!(rep.is_clean(), "{:?}", rep.problems);
    assert_eq!(rep.records.iter().map(|r| r.offset).collect::<Vec<_>>(), offs);
    assert_eq!(rep.resume_offset, rep.records.last().map(|r| r.offset + r.span).unwrap());
}

#[test]
fn corrupt_record_is_skipped_and_next_found() {
    for align in [4096, 1] {
        let (f, _a, offs) = build(align, 5);
        f.with_bytes(|b| b[offs[2] as usize + 10] ^= 0xFF); // damage header of record 2
        let rep = scan(&f, ScanOptions::default()).unwrap();
        let found: Vec<u64> = rep.records.iter().map(|r| r.offset).collect();
        assert_eq!(found, vec![offs[0], offs[1], offs[3], offs[4]], "align {align}");
        assert!(matches!(rep.problems.as_slice(), [Problem::Corrupt { offset, .. }] if *offset == offs[2]));
    }
}

#[test]
fn body_corruption_detected_only_when_verifying() {
    let (f, _a, offs) = build(4096, 3);
    f.with_bytes(|b| b[offs[1] as usize + 500] ^= 0x01);
    assert!(scan(&f, ScanOptions::default()).unwrap().is_clean());
    let rep = scan(&f, ScanOptions { verify_bodies: true }).unwrap();
    assert_eq!(rep.records.len(), 3);
    assert!(matches!(rep.problems.as_slice(), [Problem::BodyChecksum { offset, .. }] if *offset == offs[1]));
}

#[test]
fn zeroed_slot_is_reported_unwritten_not_corrupt() {
    // Simulates a writer that reserved a slot and crashed before writing it.
    let (f, a, offs) = build(4096, 4);
    let span1 = offs[2] - offs[1];
    f.with_bytes(|b| b[offs[1] as usize..offs[2] as usize].fill(0));
    let rep = scan(&f, ScanOptions::default()).unwrap();
    assert_eq!(rep.records.len(), 3);
    assert_eq!(rep.problems, vec![Problem::Unwritten { offset: offs[1], len: span1 }]);
    assert!(rep.is_clean());
    drop(a);
}

#[test]
fn truncated_tail_keeps_earlier_records() {
    let (f, _a, offs) = build(4096, 4);
    f.with_bytes(|b| b.truncate(offs[3] as usize + 100));
    let rep = scan(&f, ScanOptions::default()).unwrap();
    assert_eq!(rep.records.len(), 3);
    assert!(matches!(rep.problems.as_slice(), [Problem::Corrupt { offset, .. }] if *offset == offs[3]));
    assert_eq!(rep.resume_offset, offs[3]);
}

#[test]
fn bad_footer_falls_back_to_scan() {
    let (f, a, _) = build(4096, 3);
    let info = a.seal().unwrap();
    f.with_bytes(|b| b[info.footer_offset as usize] ^= 0xFF);
    assert!(matches!(read_sealed_index(&f), Err(ScanError::Footer(_))));
    let rep = scan(&f, ScanOptions::default()).unwrap();
    assert!(rep.sealed.is_none());
    assert_eq!(rep.records.len(), 3);
    assert!(matches!(rep.problems.first(), Some(Problem::FooterUnreadable { .. })));
}

#[test]
fn a_trailer_counting_more_entries_than_any_file_holds_is_an_error() {
    use lanstash_domain::segment::footer::Trailer;
    use lanstash_domain::segment::header::HEADER_REGION;
    let f = MemFile::default();
    let t = Trailer { footer_offset: HEADER_REGION, entry_count: 1 << 59, footer_crc: 0 }.encode();
    f.with_bytes(|b| {
        b.resize(HEADER_REGION as usize, 0);
        b.extend_from_slice(&t);
    });
    assert!(matches!(read_sealed_index(&f), Err(ScanError::Footer(_))), "not sealed with no entries");
}

#[test]
fn bad_header_is_fatal() {
    let (f, _a, _) = build(4096, 1);
    f.with_bytes(|b| b[0] = b'X');
    assert!(matches!(scan(&f, ScanOptions::default()), Err(ScanError::Header(_))));
}
