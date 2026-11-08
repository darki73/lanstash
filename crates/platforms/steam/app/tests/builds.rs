//! Tests for `lanstash_steam_app::builds`: a scripted account, its fake CDN,
//! and a store holding some of app 100's chunks.

mod support;

use std::sync::Arc;

use lanstash_app::stash_put::StashPut;
use lanstash_domain::json::{self, Value};
use lanstash_ports::clock::FixedClock;
use lanstash_steam_app::builds::SteamBuilds;
use lanstash_steam_app::library::SteamLibrary;
use lanstash_steam_domain::apps::Target;
use lanstash_steam_domain::profile::Steam;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use support::account::services;

fn store() -> Arc<Store> {
    let (s, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(5)))
            .unwrap();
    Arc::new(s)
}

/// Puts chunk `sha` of `depot` (`size` bytes) in the store.
fn put(store: &Arc<Store>, depot: u32, sha: u8, size: usize) {
    let p = StashPut::new(Arc::clone(store), Arc::new(Steam), 0);
    let target = format!("/depot/{depot}/chunk/{}", format!("{sha:02x}").repeat(20));
    p.put(&target, 0, &vec![sha; size], size as u64, &[], true).unwrap();
}

fn game(view: &str) -> Value {
    json::parse(view).unwrap().get("games").unwrap().as_array()[0].clone()
}

#[test]
fn a_stashed_app_shows_its_build_and_how_much_of_it_is_held() {
    let (s, _) = services(true);
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(0))));
    let st = store();
    let builds = SteamBuilds::new(s, Arc::clone(&lib), Arc::clone(&st), Target::default(), true);
    assert_eq!(builds.view(), "{\"region\":\"windows\",\"lancache\":true,\"games\":[]}\n", "no library yet");
    assert!(builds.refresh().is_err());
    lib.refresh().unwrap();
    builds.refresh().unwrap();
    assert!(builds.view().contains("\"games\":[]"), "nothing of app 100 is stashed");
    // Depot 101's chunks 01 (100 bytes) and 02 (200 bytes); 102's chunk 03 (40) not yet.
    put(&st, 101, 1, 100);
    let g = game(&builds.view());
    assert_eq!(g.get("game").and_then(Value::as_str), Some("100"));
    assert_eq!(g.get("version").and_then(Value::as_str), Some("555"));
    assert!(builds.view().contains("\"have\":null,\"want\":null"), "its install is read on the next refresh");
    let status = |known: bool, current: bool| lanstash_ports::platform::BuildStatus {
        game: "100".into(),
        version: "555".into(),
        known,
        current,
    };
    assert_eq!(builds.status(), [status(false, false)], "not known yet");
    builds.refresh().unwrap();
    assert!(builds.view().contains("\"have\":100,\"want\":340,\"plan_error\":null,\"current\":false"));
    put(&st, 101, 2, 200);
    put(&st, 102, 3, 40);
    assert!(builds.view().contains("\"have\":340,\"want\":340,\"plan_error\":null,\"current\":true"));
    assert_eq!(builds.status(), [status(true, true)]);
}

#[test]
fn after_a_restart_kept_chunk_lists_and_library_need_no_manifest_and_no_logon() {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    use lanstash_ports::keep::Keep;

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
    let keep: Arc<Memory> = Arc::default();
    let st = store();
    put(&st, 101, 1, 100);
    put(&st, 101, 2, 200);
    put(&st, 102, 3, 40);
    // The first run: the library read and the manifests downloaded, both kept.
    let (s, cdn) = services(true);
    let s = s.with_chunk_lists(keep.clone());
    let lib = Arc::new(SteamLibrary::new(s.clone(), Arc::new(FixedClock(1_000))).with_keep(keep.clone()));
    lib.refresh().unwrap();
    let first = SteamBuilds::new(s.clone(), Arc::clone(&lib), Arc::clone(&st), Target::default(), false);
    first.refresh().unwrap();
    assert!(first.view().contains("\"current\":true"));
    let downloaded = cdn.calls();
    assert!(downloaded >= 2, "two manifests downloaded: {downloaded}");
    // The next run: a new process with what was kept, signed out (so no logon could happen).
    let (s2, cdn2) = services(false);
    let s2 = s2.with_chunk_lists(keep.clone());
    let lib2 = Arc::new(SteamLibrary::new(s2.clone(), Arc::new(FixedClock(2_000))).with_keep(keep.clone()));
    assert_eq!(lib2.get().map(|l| l.at_ms), Some(1_000), "the library at once, as old as it is");
    let again = SteamBuilds::new(s2.clone(), Arc::clone(&lib2), Arc::clone(&st), Target::default(), false);
    let ready = again.ready();
    again.refresh().unwrap();
    assert!(again.view().contains("\"have\":340,\"want\":340,\"plan_error\":null,\"current\":true"));
    assert_eq!(cdn2.calls(), 0, "no manifest downloaded");
    assert_eq!(s2.logons(), 0, "no logon");
    assert!(!ready.load(Ordering::Acquire), "raised by the thread after its first refresh, not by a refresh");
}
