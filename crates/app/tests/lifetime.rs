//! Tests for `lanstash_app::lifetime`: totals over restarts.

mod support;

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use lanstash_app::lifetime::{KEPT, Lifetime};
use lanstash_app::serve::{Cache, ServeOptions};
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::keep::Keep;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

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

/// A cache with nothing upstream: only its counters are read.
fn cache() -> Arc<Cache> {
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(0)))
            .unwrap();
    let profiles = support::profiles::profiles();
    Arc::new(Cache::new(Arc::new(store), HostRules::default(), &profiles, Arc::new(NoEvents), ServeOptions::default()))
}

#[test]
fn the_totals_add_up_over_restarts_and_keep_when_they_began() {
    let keep: Arc<Memory> = Arc::default();
    let first = cache();
    let life = Lifetime::new(Arc::clone(&first), Arc::clone(&keep) as Arc<dyn Keep>, 5_000);
    assert_eq!(life.values(), [("served_bytes", 0), ("fetched_bytes", 0), ("since_ms", 5_000)]);
    first.counters.hit_bytes.store(300, Ordering::Relaxed);
    life.save().unwrap();
    assert_eq!(keep.load(KEPT).unwrap().unwrap(), b"served=300\nfetched=0\nsince_ms=5000\n");
    // The next start: its own counters on top of the kept totals, counted since the first start.
    let second = cache();
    let again = Lifetime::new(Arc::clone(&second), Arc::clone(&keep) as Arc<dyn Keep>, 9_000);
    second.counters.hit_bytes.store(20, Ordering::Relaxed);
    assert_eq!(again.values(), [("served_bytes", 320), ("fetched_bytes", 0), ("since_ms", 5_000)]);
}
