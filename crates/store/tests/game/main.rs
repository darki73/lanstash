//! Tests for `lanstash_store::game`: one game directory on its own, without a
//! `Store` around it.

mod evict;
mod load;
mod write;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::footer::key_hash;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::FixedClock;
use lanstash_store::game::{Game, GameReport, SegmentOptions};
use lanstash_store::index::Index;
use lanstash_store::layout::GameDir;
use lanstash_store::test_support::MemFs;

fn dir() -> GameDir {
    GameDir::new("steam", "7")
}

/// Loads the game from `fs` into a fresh index.
fn load(fs: &MemFs, read_only: bool, now_ms: u64) -> (Game, Index, GameReport) {
    let index = Index::new();
    let (game, report) =
        Game::load(dir(), Arc::new(fs.clone()), Arc::new(AtomicU64::new(0)), &index, read_only, now_ms);
    (game, index, report)
}

/// Stores `body` as a whole-object record of `key`.
fn put(game: &Game, index: &Index, opts: SegmentOptions, key: &str, body: &[u8]) {
    let (seg, r) = game.reserve_record(index, opts, &FixedClock(1), key, b"", body.len() as u64).unwrap();
    seg.appender.write_body(&r, 0, body).unwrap();
    let loc = seg.appender.commit(r, RecordFlags::default(), crc32c(body)).unwrap();
    index.insert_record(key_hash(key.as_bytes()), seg.id, loc.offset, loc.header_len, loc.body_len);
}

#[test]
fn an_empty_directory_loads_as_an_empty_game() {
    let fs = MemFs::default();
    let (game, index, report) = load(&fs, false, 5);
    assert_eq!(report, GameReport::default());
    assert!(index.is_empty());
    assert_eq!(game.dir, dir());
    assert!(game.close_active().is_none(), "nothing is written yet");
}

#[test]
fn reports_add_up_field_by_field() {
    let mut a = GameReport { records: 2, extents: 1, failed: vec![("a".into(), "x".into())], ..GameReport::default() };
    let b = GameReport { records: 3, journal_bad: 4, failed: vec![("b".into(), "y".into())], ..GameReport::default() };
    a.add(&b);
    assert_eq!((a.records, a.extents, a.journal_bad, a.failed.len()), (5, 1, 4, 2));
}
