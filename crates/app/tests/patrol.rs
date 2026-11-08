//! Tests for `lanstash_app::patrol`: the store checked in the background,
//! paced, resumed after a restart, and only lasting damage reported.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use lanstash_app::patrol::{KEPT, Patrol, PatrolOptions, Planner};
use lanstash_domain::checksum::crc::crc32c;
use lanstash_domain::segment::record::RecordFlags;
use lanstash_ports::clock::{Clock, FixedClock, Pause};
use lanstash_ports::keep::Keep;
use lanstash_store::store::{ObjectRef, Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use lanstash_store::verify::plan;

/// A clock that moves only when paused (checking takes no time).
#[derive(Default)]
struct Time(AtomicU64);

impl Clock for Time {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

impl Pause for Time {
    fn pause_ms(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keep for Memory {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), data.to_vec());
        Ok(())
    }
}

/// A store of games `1` and `2`, a segment each of three 1,000-byte records.
fn stored() -> (MemFs, Roots) {
    let fs = MemFs::default();
    let roots = Roots::single(Arc::new(fs.clone()));
    let (s, _) = Store::open(roots.clone(), StoreOptions::default(), Arc::new(FixedClock(1))).unwrap();
    for game in ["1", "2"] {
        for i in 0..3u8 {
            let key = format!("steam/{game}/{i}");
            let b = vec![i; 1000];
            let w = s.reserve_record(ObjectRef { service: "steam", game, key: &key }, b"", 1000).unwrap();
            w.write(0, &b).unwrap();
            w.commit(RecordFlags::default(), crc32c(&b)).unwrap();
        }
    }
    (fs, roots)
}

fn damage(fs: &MemFs, path: &str) {
    fs.get(path).unwrap().with_bytes(|b| {
        let n = b.len();
        b[n - 10] ^= 1;
    });
}

fn planner(roots: Roots) -> Planner {
    Box::new(move || plan(&roots).map_err(|e| e.to_string()))
}

const DAY: u64 = 86_400_000;

fn patrol(plan: Planner, time: &Arc<Time>, keep: &Arc<Memory>, rate: u64) -> Patrol {
    let options = PatrolOptions { every_ms: 7 * DAY, rate };
    Patrol::new(plan, time.clone(), time.clone(), keep.clone(), options)
}

#[test]
fn a_clean_round_is_paced_to_the_rate_and_the_next_waits_for_the_period() {
    let (_, roots) = stored();
    let time = Arc::new(Time::default());
    time.pause_ms(8 * DAY);
    let keep = Arc::new(Memory::default());
    let p = patrol(planner(roots), &time, &keep, 1_000);
    assert!(p.due(), "never ran");
    assert!(p.round().is_empty());
    let s = p.state();
    assert_eq!((s.running, s.items_done, s.items), (false, 2, 2));
    assert!(s.bytes > 6_000, "two segments of three records and their headers: {}", s.bytes);
    assert_eq!(s.finished_ms - s.started_ms, s.bytes, "at 1,000 bytes a second: a millisecond a byte");
    assert!(!p.due());
    time.pause_ms(7 * DAY);
    assert!(p.due(), "a period later");
    let values: BTreeMap<_, _> = p.values().into_iter().collect();
    assert_eq!((values["problems"], values["running"], values["items"]), (0, 0, 2));
}

#[test]
fn damage_is_reported_and_kept_and_a_new_patrol_knows_it() {
    let (fs, roots) = stored();
    damage(&fs, "steam/2/seg/00000.lcseg");
    let time = Arc::new(Time::default());
    let keep = Arc::new(Memory::default());
    let found = patrol(planner(roots.clone()), &time, &keep, 0).round();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, "steam/2/seg/00000.lcseg");
    let again = patrol(planner(roots), &time, &keep, 0).state();
    assert_eq!(again.problems, found, "kept between runs");
    assert!(keep.load(KEPT).unwrap().is_some());
}

#[test]
fn what_is_fine_on_a_second_look_is_not_reported() {
    // The first listing reads a damaged copy; by the second look the file is whole (rewritten, or gone and back).
    let (bad, bad_roots) = stored();
    damage(&bad, "steam/1/seg/00000.lcseg");
    let (_, good_roots) = stored();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let plan: Planner = Box::new(move || {
        let roots = if counted.fetch_add(1, Ordering::Relaxed) == 0 { &bad_roots } else { &good_roots };
        plan(roots).map_err(|e| e.to_string())
    });
    let time = Arc::new(Time::default());
    let found = patrol(plan, &time, &Arc::new(Memory::default()), 0).round();
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(calls.load(Ordering::Relaxed), 2, "listed again to confirm");
}

#[test]
fn a_round_cut_short_resumes_after_its_last_item() {
    let (_, roots) = stored();
    let time = Arc::new(Time::default());
    let keep = Arc::new(Memory::default());
    keep.save(KEPT, b"started_ms=5\nfinished_ms=0\ncursor=steam/1/seg/00000.lcseg\nproblem=x\tearlier\n").unwrap();
    let p = patrol(planner(roots), &time, &keep, 0);
    assert!(p.due(), "cut short");
    let found = p.round();
    let s = p.state();
    assert_eq!((s.items, s.started_ms), (1, 5), "only game 2 is left; the round keeps its start");
    assert!(found.is_empty(), "an earlier find that a fresh listing does not have is dropped: {found:?}");
    assert!(s.cursor.is_empty());
}

#[test]
fn a_store_that_cannot_be_listed_is_a_problem() {
    let time = Arc::new(Time::default());
    let p = patrol(Box::new(|| Err("no roots".to_owned())), &time, &Arc::new(Memory::default()), 0);
    assert_eq!(p.round(), [("store".to_owned(), "no roots".to_owned())]);
}

#[test]
fn a_round_that_could_not_list_keeps_its_place_and_stays_due() {
    let time = Arc::new(Time::default());
    let keep = Arc::new(Memory::default());
    keep.save(KEPT, b"started_ms=5\nfinished_ms=0\ncursor=steam/1/seg/00000.lcseg\n").unwrap();
    let p = patrol(Box::new(|| Err("the share is away".to_owned())), &time, &keep, 0);
    p.round();
    let s = p.state();
    assert_eq!((s.cursor.as_str(), s.finished_ms), ("steam/1/seg/00000.lcseg", 0));
    assert!(p.due(), "tried again at the next look, not a whole period later");
}
