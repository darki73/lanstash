//! Putting bytes from elsewhere into the stash where a download would put them.

mod support;

use std::sync::Arc;

use lanstash_app::stash_put::StashPut;
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_ports::clock::FixedClock;
use lanstash_store::index::{Lookup, StoredObject};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use support::profiles::Ranged;

fn store() -> Arc<Store> {
    let (s, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    Arc::new(s)
}

fn found(s: &Store, target: &str) -> StoredObject {
    match s.index().lookup(&object_key("blizzard", target)) {
        Lookup::Found(o) => o,
        other => panic!("{target}: {other:?}"),
    }
}

#[test]
fn small_whole_files_are_records_the_rest_extents() {
    let s = store();
    let put = StashPut::new(Arc::clone(&s), Arc::new(Ranged), 100);
    // Whole and small: one record of the game.
    put.put("/tpr/ovw/data/ab/cd/loose", 0, b"loose file", 10, b"content-type=x\n", true).unwrap();
    let o = found(&s, "/tpr/ovw/data/ab/cd/loose");
    assert!(o.covers(Span::new(0, 10)));
    assert_eq!(o.total, 10);
    // A range of a large archive: an extent of its file.
    put.put("/tpr/ovw/data/ab/cd/archive", 500, &[7u8; 50], 1000, b"", true).unwrap();
    let a = found(&s, "/tpr/ovw/data/ab/cd/archive");
    assert_eq!(a.total, 1000);
    assert!(a.covers(Span::new(500, 550)));
    assert!(!a.covers(Span::new(0, 10)));
}

#[test]
fn bad_writes_are_refused() {
    let s = store();
    let put = StashPut::new(Arc::clone(&s), Arc::new(Ranged), 100);
    assert!(put.put("/tpr/ovw/data/ab/cd/x", 990, &[1u8; 20], 1000, b"", true).is_err(), "past the end");
    assert!(put.put("/tpr/ovw/data/ab/cd/x", 0, b"", 1000, b"", true).is_err(), "nothing");
}

#[test]
fn parts_count_only_once_committed() {
    let s = store();
    let put = StashPut::new(Arc::clone(&s), Arc::new(Ranged), 100);
    // A large file in parts: written as they come, invisible until committed.
    let mut p = put.begin("/tpr/ovw/data/ab/cd/big", 0, 300, 300, b"").unwrap();
    p.write(0, &[1u8; 100]).unwrap();
    assert!(p.write(150, &[1u8; 10]).is_err(), "out of order");
    p.write(100, &[2u8; 200]).unwrap();
    assert!(!found(&s, "/tpr/ovw/data/ab/cd/big").covers(Span::new(0, 1)), "no byte counts yet");
    p.commit(true).unwrap();
    assert!(found(&s, "/tpr/ovw/data/ab/cd/big").covers(Span::new(0, 300)));
    // A small whole file: gathered, one record on commit.
    let mut small = put.begin("/tpr/ovw/data/ab/cd/small", 0, 20, 20, b"").unwrap();
    small.write(0, &[3u8; 8]).unwrap();
    small.write(8, &[4u8; 12]).unwrap();
    small.commit(true).unwrap();
    assert!(found(&s, "/tpr/ovw/data/ab/cd/small").covers(Span::new(0, 20)));
    // Dropped uncommitted, or committed short: nothing.
    let mut dropped = put.begin("/tpr/ovw/data/ab/cd/dropped", 0, 300, 300, b"").unwrap();
    dropped.write(0, &[5u8; 300]).unwrap();
    drop(dropped);
    assert!(
        !matches!(s.lookup(&object_key("blizzard", "/tpr/ovw/data/ab/cd/dropped")), Lookup::Found(o) if o.covers(Span::new(0, 300)))
    );
    let mut short = put.begin("/tpr/ovw/data/ab/cd/short", 0, 300, 300, b"").unwrap();
    short.write(0, &[6u8; 10]).unwrap();
    assert!(short.commit(true).is_err());
    assert!(put.begin("/tpr/ovw/data/ab/cd/x", 10, 5, 300, b"").is_err(), "an empty range");
}
