//! Tests for `lanstash_domain::segment::layout`.

use lanstash_domain::segment::header::HEADER_REGION;
use lanstash_domain::segment::layout::*;

#[test]
fn slots_are_aligned_and_disjoint() {
    let mut l = SegmentLayout::new(4096, 1 << 30).unwrap();
    let a = l.reserve(120, 1_055_104).unwrap();
    let b = l.reserve(120, 10).unwrap();
    assert_eq!(a.offset, HEADER_REGION);
    assert_eq!(a.offset % 4096, 0);
    assert_eq!(b.offset, a.offset + a.span);
    assert_eq!(b.offset % 4096, 0);
    assert_eq!(b.span, 4096);
    assert_eq!(l.records(), 2);
    assert_eq!(l.end(), b.offset + b.span);
}

#[test]
fn unaligned_layout_packs_tightly() {
    let mut l = SegmentLayout::new(1, 1 << 20).unwrap();
    let a = l.reserve(50, 100).unwrap();
    let b = l.reserve(50, 100).unwrap();
    assert_eq!(b.offset, a.offset + 150);
}

#[test]
fn full_after_max_but_first_record_always_fits() {
    let mut l = SegmentLayout::new(4096, HEADER_REGION + 8192).unwrap();
    let huge = l.reserve(100, 10 << 20).unwrap(); // bigger than max, but first
    assert_eq!(huge.offset, HEADER_REGION);
    assert_eq!(l.reserve(100, 1), Err(SegmentFull));

    let mut l = SegmentLayout::new(4096, HEADER_REGION + 8192).unwrap();
    l.reserve(100, 100).unwrap();
    l.reserve(100, 100).unwrap();
    assert_eq!(l.reserve(100, 100), Err(SegmentFull));
}

#[test]
fn rejects_bad_config() {
    assert!(SegmentLayout::new(4096, HEADER_REGION).is_err());
    assert!(SegmentLayout::new(6, 1 << 30).is_err());
    assert!(SegmentLayout::resume(4096, 1 << 30, 100, 0).is_err());
    let l = SegmentLayout::resume(4096, 1 << 30, 5000, 1).unwrap();
    assert_eq!(l.end(), 8192);
}
