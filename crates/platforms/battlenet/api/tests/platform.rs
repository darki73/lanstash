//! Battle.net as a platform: which views it has as it is set up, and what
//! they answer.

use std::sync::Arc;
use std::time::{Duration, Instant};

use lanstash_app::prefill::{PrefillOptions, Prefiller};
use lanstash_app::serve::{Cache, RequestHandler, ServeOptions};
use lanstash_app::test_support::FakeUpstream;
use lanstash_battlenet_api::platform::{BattleNet, ID};
use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildOptions};
use lanstash_battlenet_app::catalog::{BlizzardCatalog, CatalogOptions};
use lanstash_battlenet_app::nginx::{NginxProbe, ProbeOptions};
use lanstash_battlenet_app::plans::{BlizzardPlans, PlanOptions};
use lanstash_battlenet_app::prefill::BlizzardJobs;
use lanstash_battlenet_app::products::{BlizzardProducts, ProductList, ProductOptions};
use lanstash_battlenet_domain::catalog::{Install, Product};
use lanstash_domain::service::Profiles;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::platform::{Platform, ViewError};
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillState, PrefillStatus, StartError};
use lanstash_ports::storage::{FileRead, StoreError};
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

struct World {
    up: Arc<FakeUpstream>,
    clock: Arc<FixedClock>,
    store: Arc<Store>,
}

fn world() -> World {
    let clock = Arc::new(FixedClock(5));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    World { up: Arc::new(FakeUpstream::default()), clock, store: Arc::new(store) }
}

impl World {
    fn catalog(&self) -> Arc<BlizzardCatalog> {
        Arc::new(BlizzardCatalog::new(self.up.clone(), self.clock.clone(), CatalogOptions::default()))
    }

    fn builds(&self) -> BlizzardBuilds {
        let opts = BuildOptions { region: "eu".into(), ..BuildOptions::default() };
        BlizzardBuilds::new(self.up.clone(), Arc::clone(&self.store), self.clock.clone(), opts)
    }

    fn cache(&self) -> Arc<dyn RequestHandler> {
        Arc::new(Cache::new(
            Arc::clone(&self.store),
            HostRules::default(),
            &Profiles::default(),
            Arc::new(NoEvents),
            ServeOptions::default(),
        ))
    }
}

#[test]
fn only_the_catalog_is_there_until_more_is_set_up() {
    let w = world();
    let p = BattleNet::new(w.catalog());
    assert_eq!((p.id(), ID), ("blizzard", "blizzard"));
    assert!(p.names().is_some() && p.art().is_some(), "names and art come from the catalog");
    assert!(p.builds().is_none() && p.catalog().is_none() && p.prefill().is_none());
    assert!(p.game("osi").is_none() && p.nginx("osi").is_none());
}

#[test]
fn builds_and_a_game_answer_per_region() {
    let w = world();
    let p = BattleNet::new(w.catalog()).with_builds(Arc::new(w.builds()));
    assert_eq!(p.builds().unwrap().unwrap(), "{\"region\":\"eu\",\"lancache\":false,\"games\":[]}\n");
    // The fake patch service knows no product.
    let g = p.game("osi").unwrap().unwrap();
    assert!(g.starts_with("{\"game\":\"osi\",\"product\":null,\"region\":\"eu\",\"build\":null"), "{g}");
    assert!(g.ends_with(",\"lancache\":false,\"installs\":null,\"flavours\":[]}\n"), "{g}");
}

/// A prefill that can take pieces from lancache, and does nothing.
#[derive(Debug)]
struct FromLancache;

impl Prefill for FromLancache {
    fn start(&self, _: &str, _: bool, _: PrefillFrom) -> Result<(), StartError> {
        Ok(())
    }
    fn lancache(&self) -> bool {
        true
    }
    fn stop(&self) {}
    fn status(&self) -> Option<PrefillStatus> {
        None
    }
}

#[test]
fn the_views_say_whether_a_prefill_can_take_from_lancache() {
    let w = world();
    let p = BattleNet::new(w.catalog()).with_builds(Arc::new(w.builds())).with_prefill(Arc::new(FromLancache));
    assert!(p.builds().unwrap().unwrap().contains("\"lancache\":true"));
    assert!(p.game("osi").unwrap().unwrap().ends_with(",\"lancache\":true,\"installs\":null,\"flavours\":[]}\n"));
}

/// An nginx cache that cannot be read.
#[derive(Debug)]
struct Gone;

impl NginxCacheSource for Gone {
    fn list_files(&self, _: &[String]) -> Result<Vec<String>, StoreError> {
        Err(StoreError::other("share gone"))
    }
    fn open(&self, _: &str) -> Result<Arc<dyn FileRead>, StoreError> {
        Err(StoreError::other("share gone"))
    }
}

#[test]
fn the_nginx_view_is_unavailable_for_an_unknown_product() {
    let w = world();
    let builds = Arc::new(w.builds());
    let probe = NginxProbe::new(Arc::clone(&builds), Arc::new(Gone), ProbeOptions::default());
    let p = BattleNet::new(w.catalog()).with_builds(builds).with_nginx(Arc::new(probe));
    assert!(matches!(p.nginx("osi"), Some(Err(ViewError::Unavailable(_)))));
    assert!(p.game("osi").unwrap().is_ok(), "the game itself still answers");
}

struct One;
impl ProductList for One {
    fn products(&self) -> Result<Vec<Product>, String> {
        Ok(vec![Product {
            codes: vec!["s1".into()],
            installs: vec![Install::new("s1", Some("StarCraft".into()))],
            name: Some("StarCraft".into()),
            icon: None,
            key_art: None,
            background: None,
            logo: None,
        }])
    }
}

struct NotYet;
impl ProductList for NotYet {
    fn products(&self) -> Result<Vec<Product>, String> {
        Err("no catalog yet".into())
    }
}

#[test]
fn the_catalog_lists_every_product_or_is_unavailable() {
    let w = world();
    let products = |l: Arc<dyn ProductList>| {
        Arc::new(BlizzardProducts::new(l, w.up.clone(), w.clock.clone(), ProductOptions::default()))
    };
    let p = BattleNet::new(w.catalog()).with_products(products(Arc::new(One)));
    assert_eq!(
        p.catalog().unwrap().unwrap(),
        "{\"products\":[{\"code\":\"s1\",\"name\":\"StarCraft\",\"family\":\"StarCraft\",\"game\":null,\"version\":null,\"known\":false}]}\n"
    );
    let p = BattleNet::new(w.catalog()).with_products(products(Arc::new(NotYet)));
    assert!(matches!(p.catalog(), Some(Err(ViewError::Unavailable(_)))));
}

#[test]
fn prefill_is_the_platform_s_own() {
    let w = world();
    let plans =
        BlizzardPlans::new(w.cache(), Arc::clone(&w.store), w.up.clone(), w.clock.clone(), PlanOptions::default());
    let builds = Arc::new(w.builds().with_plans(Arc::new(plans)));
    let planner = Arc::new(BlizzardJobs::new(builds));
    let opts = PrefillOptions::default();
    let prefill = Prefiller::new(planner, ID, w.cache(), Arc::clone(&w.store), w.clock.clone(), opts);
    let p = BattleNet::new(w.catalog()).with_prefill(Arc::new(prefill));
    let jobs = p.prefill().unwrap();
    assert_eq!(jobs.status(), None, "none yet");
    assert!(!jobs.lancache());
    jobs.start("osi", true, PrefillFrom::Cdn).unwrap();
    // The fake patch service knows no osi: the prefill fails, and says so.
    let until = Instant::now() + Duration::from_secs(5);
    let status = loop {
        let s = jobs.status().unwrap();
        if s.state != PrefillState::Running || Instant::now() > until {
            break s;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!((status.game.as_str(), status.prerelease, status.state), ("osi", true, PrefillState::Failed));
    jobs.stop();
}
