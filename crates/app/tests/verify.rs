//! Tests for `lanstash_app::verify`.

use lanstash_app::verify::{GameVerify, Paced, VerifyProgress, verify, verify_paced};
use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::{FixedClock, Pause};
use lanstash_store::layout::GameDir;
use lanstash_store::store::{ObjectRef, Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use lanstash_store::verify::plan;
use std::sync::{Arc, Mutex};

#[test]
fn checks_every_game_in_parallel_and_reports_per_game() {
    let fs = MemFs::default();
    let roots = Roots::single(Arc::new(fs.clone()));
    let (s, _) = Store::open(roots.clone(), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    for (game, n) in [("1", 3u8), ("2", 5), ("3", 1)] {
        for i in 0..n {
            let key = format!("steam/{game}/{i}");
            let b = vec![i; 1000];
            let w = s.reserve_record(ObjectRef { service: "steam", game, key: &key }, b"", 1000).unwrap();
            w.write(0, &b).unwrap();
            w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
        }
    }
    // Damage the last record of game 2.
    fs.get("steam/2/seg/00000.lcseg").unwrap().with_bytes(|b| {
        let n = b.len();
        b[n - 10] ^= 1;
    });

    let seen = Mutex::new(Vec::new());
    let report = verify(plan(&roots).unwrap(), 3, &|p| seen.lock().unwrap().push(p));

    let g = |id: &str| report.games[&GameDir::new("steam", id)];
    assert_eq!(g("1"), GameVerify { segments: 1, records: 3, record_bytes: 3000, ..GameVerify::default() });
    assert_eq!((g("2").records, g("2").problems), (4, 1));
    assert_eq!(g("3").records, 1);
    assert!(!report.is_clean());
    assert_eq!(report.problems.len(), 1);
    assert_eq!(report.problems[0].0, "steam/2/seg/00000.lcseg");

    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.len(), 3);
    let last = seen.iter().max_by_key(|p| p.items_done).unwrap();
    assert_eq!((last.items_done, last.items), (3, 3));
    assert_eq!(last.bytes_done, last.bytes);
    assert!(seen.iter().all(|p: &VerifyProgress| p.bytes > 0));
}

/// Notes every wait asked of it, and does not wait.
#[derive(Default)]
struct Waits(Mutex<Vec<u64>>);

impl Pause for Waits {
    fn pause_ms(&self, ms: u64) {
        self.0.lock().unwrap().push(ms);
    }
}

#[test]
fn a_paced_check_waits_until_its_bytes_are_within_the_rate() {
    let fs = MemFs::default();
    let roots = Roots::single(Arc::new(fs.clone()));
    let (s, _) = Store::open(roots.clone(), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    for game in ["1", "2"] {
        let key = format!("steam/{game}/a");
        let w = s.reserve_record(ObjectRef { service: "steam", game, key: &key }, b"", 1000).unwrap();
        w.write(0, &[7; 1000]).unwrap();
        w.commit(RecordFlags::default(), crc32c(&[7; 1000])).unwrap();
    }
    let p = plan(&roots).unwrap();
    let bytes: u64 = p.items.iter().map(|i| i.bytes).sum();
    let waits = Waits::default();
    // Half the bytes a second, on a clock that stands still: the whole check takes 2 s.
    let paced = Paced { bytes_per_s: bytes / 2, clock: &FixedClock(0), pause: &waits };
    let report = verify_paced(p, 1, Some(&paced), &|_| {});
    assert!(report.is_clean());
    let waits = waits.0.into_inner().unwrap();
    assert_eq!(waits.len(), 2, "after each item");
    assert_eq!(waits.last(), Some(&2000));
}
