//! Tests for `lanstash_epic_app::builds`: with nothing of Epic stashed, nothing to say.

use std::sync::Arc;

use lanstash_epic_app::accounts::{EpicAccounts, Wiring};
use lanstash_epic_app::builds::EpicBuilds;
use lanstash_epic_app::library::EpicLibrary;
use lanstash_epic_app::prefill::EpicPlanner;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::{Web, WebError, WebRequest, WebResponse};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

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

#[test]
fn with_nothing_of_epic_stashed_the_view_has_no_games() {
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
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(5)))
            .unwrap();
    let builds = EpicBuilds::new(library, planner, Arc::new(store));
    builds.refresh();
    assert_eq!(builds.view(), "{\"region\":\"windows\",\"lancache\":false,\"games\":[]}\n");
    assert!(builds.status().is_empty());
}
