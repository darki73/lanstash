//! Tests for `lanstash_store::segment::writer`.

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::header::HEADER_REGION;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::storage::FileRead;
use lanstash_store::segment::append_error::AppendError;
use lanstash_store::segment::scan::{ScanOptions, read_sealed_index, scan};
use lanstash_store::segment::writer::*;
use lanstash_store::test_support::{FileOp, MemFile, rec, sample_header};
use std::sync::Arc;

#[test]
fn appends_are_aligned_and_sequential() {
    let a = SegmentAppender::create(MemFile::default(), sample_header(4096, 1 << 20)).unwrap();
    let l1 = a.append(rec("k1"), &[1u8; 5000]).unwrap();
    let l2 = a.append(rec("k2"), &[2u8; 10]).unwrap();
    assert_eq!(l1.offset, HEADER_REGION);
    assert_eq!(l2.offset, l1.offset + l1.span);
    assert_eq!(l2.offset % 4096, 0);
    assert_eq!(a.record_count(), 2);
}

#[test]
fn full_segment_reports_full() {
    let a = SegmentAppender::create(MemFile::default(), sample_header(4096, HEADER_REGION + 8192)).unwrap();
    a.append(rec("a"), &[0u8; 100]).unwrap();
    a.append(rec("b"), &[0u8; 100]).unwrap();
    assert!(matches!(a.append(rec("c"), &[0u8; 100]), Err(AppendError::Full)));
}

#[test]
fn seal_then_no_more_appends() {
    let a = SegmentAppender::create(MemFile::default(), sample_header(4096, 1 << 20)).unwrap();
    a.append(rec("a"), b"hello").unwrap();
    let info = a.seal().unwrap();
    assert_eq!(info.entry_count, 1);
    assert!(matches!(a.append(rec("b"), b"x"), Err(AppendError::Sealed)));
    assert!(matches!(a.seal(), Err(AppendError::Sealed)));
    assert_eq!(a.into_inner().bytes().len() as u64, info.file_len);
}

#[test]
fn the_records_are_synced_before_the_footer_is_written() {
    let file = MemFile::default();
    let a = SegmentAppender::create(file.clone(), sample_header(4096, 1 << 20)).unwrap();
    a.append(rec("a"), &[1u8; 5000]).unwrap();
    let info = a.seal().unwrap();
    let ops = file.ops();
    let footer = ops
        .iter()
        .position(|o| matches!(o, FileOp::Write { offset, .. } if *offset == info.footer_offset))
        .expect("the footer was written");
    let last_record = ops[..footer].iter().rposition(|o| matches!(o, FileOp::Write { .. })).unwrap();
    assert!(ops[last_record..footer].contains(&FileOp::Sync), "{ops:?}");
    assert_eq!(ops.last(), Some(&FileOp::Sync), "and the footer itself after it");
}

#[test]
fn failed_write_does_not_enter_footer() {
    let file = MemFile::default();
    let a = SegmentAppender::create(file.clone(), sample_header(4096, 1 << 20)).unwrap();
    file.fail_writes(true);
    assert!(matches!(a.append(rec("a"), b"x"), Err(AppendError::Store(_))));
    file.fail_writes(false);
    a.append(rec("b"), b"y").unwrap();
    assert_eq!(a.record_count(), 1);
    a.seal().unwrap(); // nothing left in flight
}

#[test]
fn parallel_appends_get_disjoint_slots() {
    let a = Arc::new(SegmentAppender::create(MemFile::default(), sample_header(4096, 64 << 20)).unwrap());
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let a = Arc::clone(&a);
            std::thread::spawn(move || {
                (0..50)
                    .map(|i| a.append(rec(&format!("t{t}/r{i}")), &vec![t as u8; 1000 + i]).unwrap())
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut locs: Vec<RecordLocation> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    locs.sort_by_key(|l| l.offset);
    assert_eq!(locs.len(), 400);
    for w in locs.windows(2) {
        assert!(w[0].offset + w[0].span <= w[1].offset, "overlap at {}", w[1].offset);
    }
}

#[test]
fn a_reservation_is_filled_in_pieces_and_readable_before_commit() {
    let file = MemFile::default();
    let a = SegmentAppender::create(file.clone(), sample_header(4096, 1 << 20)).unwrap();
    let body: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
    let r = a.reserve("k", b"m=1\n", body.len() as u64).unwrap();
    for (i, part) in body.chunks(777).enumerate() {
        a.write_body(&r, (i * 777) as u64, part).unwrap();
    }
    assert!(a.write_body(&r, 9_999, b"xx").is_err(), "past the reservation");
    let mut back = vec![0u8; 100];
    a.file().read_exact_at(r.body_offset() + 500, &mut back).unwrap();
    assert_eq!(back, body[500..600]);
    assert!(scan(&file, ScanOptions::default()).unwrap().records.is_empty(), "no header yet: no record");
    let loc = a.commit(r, RecordFlags::default(), crc32c(&body)).unwrap();
    let rep = scan(&file, ScanOptions { verify_bodies: true }).unwrap();
    assert_eq!(rep.records.len(), 1);
    assert_eq!((rep.records[0].offset, rep.records[0].body_ok), (loc.offset, Some(true)));
    assert_eq!(rep.records[0].header.meta, b"m=1\n");
}

#[test]
fn closing_with_open_reservations_seals_when_the_last_one_ends() {
    let file = MemFile::default();
    let a = SegmentAppender::create(file.clone(), sample_header(4096, 1 << 20)).unwrap();
    let r1 = a.reserve("one", b"", 3).unwrap();
    let r2 = a.reserve("two", b"", 3).unwrap();
    assert_eq!(a.close().unwrap(), None, "deferred");
    assert!(matches!(a.reserve("three", b"", 3), Err(AppendError::Full)));
    a.write_body(&r1, 0, b"abc").unwrap();
    a.commit(r1, RecordFlags::default(), crc32c(b"abc")).unwrap();
    assert!(!a.is_sealed() && a.deferred_seal().is_none());
    a.abandon(r2);
    assert!(a.is_sealed());
    let info = a.deferred_seal().unwrap().unwrap();
    assert_eq!(info.entry_count, 1);
    assert_eq!(read_sealed_index(&file).unwrap().unwrap().entries.len(), 1);
    assert_eq!(a.seal_when_idle().unwrap(), info);
}

#[test]
fn closing_an_idle_segment_seals_at_once() {
    let a = SegmentAppender::create(MemFile::default(), sample_header(4096, 1 << 20)).unwrap();
    a.append(rec("a"), b"x").unwrap();
    assert_eq!(a.close().unwrap().map(|i| i.entry_count), Some(1));
    assert!(matches!(a.close(), Err(AppendError::Sealed)));
}
