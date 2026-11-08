//! Tests for `lanstash_epic_app::sizes`: the kept form, the size of a build as
//! kept (and none for another build), and nothing to work out without a library.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lanstash_epic::library::Asset;
use lanstash_epic_app::accounts::{EpicAccounts, Wiring};
use lanstash_epic_app::library::EpicLibrary;
use lanstash_epic_app::prefill::EpicPlanner;
use lanstash_epic_app::sizes::{EpicSizes, KEPT, decode, encode};
use lanstash_ports::clock::FixedClock;
use lanstash_ports::keep::Keep;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};

struct NoSecrets;

impl Secrets for NoSecrets {
    fn load(&self, _name: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn save(&self, _name: &str, _value: &str) -> Result<(), String> {
        Ok(())
    }
    fn remove(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }
}

struct Offline;

impl Web for Offline {
    fn send(&self, _req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        Err("offline".into())
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

fn asset(app: &str, build: &str) -> Asset {
    Asset { app_name: app.into(), namespace: "n".into(), catalog_item_id: "c".into(), build_version: build.into() }
}

fn sizes(keep: Arc<Memory>) -> EpicSizes {
    let web = Arc::new(Offline);
    let wiring = Wiring {
        web: web.clone(),
        secrets: Arc::new(NoSecrets),
        clock: Arc::new(FixedClock(1)),
        client_id: "id".into(),
        client_secret: "s".into(),
    };
    let library = Arc::new(EpicLibrary::new(Arc::new(EpicAccounts::new(wiring)), web.clone(), Arc::new(FixedClock(1))));
    let planner = Arc::new(EpicPlanner::new(Arc::clone(&library), web));
    EpicSizes::new(library, planner).with_keep(keep)
}

#[test]
fn the_kept_form_round_trips_and_skips_what_does_not_parse() {
    let mut s = BTreeMap::new();
    s.insert("app1".to_string(), ("v1".to_string(), Ok(4_091_159_893)));
    s.insert("app2".to_string(), ("v2".to_string(), Err("no manifest".to_string())));
    let text = encode(&s);
    assert_eq!(text, "app1\tv1\t4091159893\n", "a size that could not be worked out is not kept");
    let back = decode(&format!("{text}broken line\napp3\tv3\tlots\n"));
    assert_eq!(back.len(), 1);
    assert_eq!(back.get("app1"), Some(&("v1".to_string(), Ok(4_091_159_893))));
}

#[test]
fn a_kept_size_is_its_builds_and_nothing_is_worked_out_without_a_library() {
    let keep = Arc::new(Memory::default());
    keep.save(KEPT, b"app1\tv1\t500\n").unwrap();
    let s = sizes(keep);
    assert_eq!(s.size(&asset("app1", "v1")), Some(500));
    assert_eq!(s.size(&asset("app1", "v2")), None, "a new build is worked out again");
    assert_eq!(s.size(&asset("app9", "v1")), None);
    assert!(!s.one(), "no library: no game to work out");
}
