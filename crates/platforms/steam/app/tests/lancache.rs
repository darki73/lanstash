//! Tests for `lanstash_steam_app::lancache`: the scripted account's app 100
//! (depot 101: chunks 01 of 100 bytes and 02 of 200; depot 102: chunk 03 of
//! 40), and a lancache directory holding some of their first slices.

mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use lanstash_ports::clock::{FixedClock, NoPause};
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::storage::{FileRead, StoreError, StoreErrorKind};
use lanstash_steam_app::lancache::SteamLancache;
use lanstash_steam_app::library::SteamLibrary;
use lanstash_steam_domain::apps::Target;
use lanstash_steam_domain::manifest::Chunk;
use lanstash_steam_domain::share::first_slice;
use support::account::services;

/// A lancache directory that has exactly these files (none can be read).
struct Holds(BTreeSet<String>);

impl NginxCacheSource for Holds {
    fn list_files(&self, _: &[String]) -> Result<Vec<String>, StoreError> {
        Ok(self.0.iter().cloned().collect())
    }
    fn open(&self, rel: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Err(StoreError::new(StoreErrorKind::NotFound, rel))
    }
    fn exists(&self, rel: &str) -> Result<bool, StoreError> {
        Ok(self.0.contains(rel))
    }
}

fn slice_of(depot: u32, sha: u8) -> String {
    first_slice(depot, &Chunk { sha: [sha; 20], crc: 0, offset: 0, original: 0, compressed: 0 })
}

#[test]
fn what_lancache_holds_of_each_game_is_estimated_once_per_build() {
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    let holds = Holds([slice_of(101, 2), slice_of(102, 3)].into_iter().collect());
    let probe = SteamLancache::new(s, Arc::clone(&lib), Arc::new(holds), Target::default());
    assert!(probe.pass(&NoPause).is_err(), "no library yet");
    lib.refresh().unwrap();
    let app = lib.get().unwrap().app(100).unwrap().clone();
    let shelf = lib.get().unwrap();
    assert!(probe.share(&shelf, &app).is_none());
    assert_eq!(probe.pass(&NoPause), Ok(1));
    // Every chunk sampled (three): 240 of 340 bytes held.
    assert_eq!(probe.share(&shelf, &app), Some((240, 340)));
    assert_eq!(probe.pass(&NoPause), Ok(0), "the same build is not probed again");
    let newer = lanstash_steam_domain::apps::App { build: Some(556), ..app };
    assert!(probe.share(&shelf, &newer).is_none(), "another build: not known");
}

#[derive(Default)]
struct Kept(std::sync::Mutex<Option<Vec<u8>>>);

impl lanstash_ports::keep::Keep for Kept {
    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        assert_eq!(name, lanstash_steam_app::lancache::KEPT);
        *self.0.lock().unwrap() = Some(data.to_vec());
        Ok(())
    }
}

#[test]
fn the_results_outlive_a_restart() {
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    lib.refresh().unwrap();
    let kept = Arc::new(Kept::default());
    let holds = || Arc::new(Holds([slice_of(101, 2)].into_iter().collect()));
    let first = SteamLancache::new(s.clone(), Arc::clone(&lib), holds(), Target::default()).with_keep(kept.clone());
    assert_eq!(first.pass(&NoPause), Ok(1));
    assert_eq!(kept.0.lock().unwrap().as_deref(), Some(&b"100 555 200 340\n"[..]));
    // After a restart: known at once, and not probed again.
    let again = SteamLancache::new(s, Arc::clone(&lib), holds(), Target::default()).with_keep(kept);
    let app = lib.get().unwrap().app(100).unwrap().clone();
    assert_eq!(again.share(&lib.get().unwrap(), &app), Some((200, 340)));
    assert_eq!(again.pass(&NoPause), Ok(0));
}

#[test]
fn kept_lines_read_back_and_bad_ones_are_skipped() {
    use lanstash_steam_app::lancache::{Share, decode, encode};
    let text = "1 - 0 0\n2 7 5 10\nnot a line\n3 x 1 1\n";
    let r = decode(text);
    assert_eq!(r.len(), 2);
    assert_eq!(r.get(&2), Some(&Share { build: Some(7), have: 5, want: 10 }));
    assert_eq!(encode(&r), "1 - 0 0\n2 7 5 10\n");
}
