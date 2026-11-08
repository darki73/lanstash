//! Tests for `lanstash_store::index::records`: a segment's records, which copy
//! an object is served from, and moving objects to their copies.

use lanstash_domain::segment::footer::key_hash;
use lanstash_store::index::records::RecordLoc;
use lanstash_store::index::*;

use crate::seg;

#[test]
fn lists_a_segments_records_and_tells_the_copy_served() {
    let idx = Index::new();
    let s = seg(&idx, "1");
    idx.insert_record(key_hash(b"k"), s, 4096, 10, 100);
    idx.insert_record(key_hash(b"k"), s, 8192, 10, 100); // a second copy: not served
    idx.insert_record(key_hash(b"j"), s, 12288, 10, 50);
    let r = idx.records_in(s);
    assert_eq!(r.iter().map(|r| r.offset).collect::<Vec<_>>(), [4096, 8192, 12288]);
    assert_eq!(r.iter().map(|r| idx.serves(r)).collect::<Vec<_>>(), [true, false, true]);
    assert!(idx.records_in(99).is_empty());
}

#[test]
fn replacing_a_segment_serves_the_copies_and_forgets_the_rest() {
    let idx = Index::new();
    let (old, new) = (seg(&idx, "1"), seg(&idx, "1"));
    idx.insert_record(key_hash(b"keep"), old, 4096, 10, 100);
    idx.insert_record(key_hash(b"drop"), old, 8192, 10, 100);
    idx.insert_record(key_hash(b"keep"), new, 4096, 10, 100); // the copy, written while the old one serves
    let copy = RecordLoc { hash: key_hash(b"keep"), seg: new, offset: 4096, header_len: 10, body_len: 100 };
    assert!(!idx.serves(&copy));
    assert!(idx.replace_segment(old, &[copy]).is_some());
    assert!(idx.serves(&copy));
    let Lookup::Found(o) = idx.lookup("keep") else { panic!() };
    assert!(matches!(&o.at(0).unwrap().1, Source::Record { seg, offset: 4096, .. } if *seg == new));
    assert!(matches!(idx.lookup("drop"), Lookup::Missing));
    assert!(idx.segment(old).is_none() && idx.replace_segment(old, &[]).is_none(), "gone once");
}
