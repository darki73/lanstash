//! Every Battle.net game there is: the catalog's products with their CDN
//! directories and versions, looked up in the background; and builds of a
//! game not in the stash yet, found through its directory.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildOptions};
use lanstash_battlenet_app::products::{BlizzardProducts, CatalogProduct, ProductList, ProductOptions};
use lanstash_battlenet_domain::catalog::{Install, Product};
use lanstash_battlenet_domain::tact::PATCH_HOST;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;
use support::Tick;

struct Fixed(Vec<Product>);
impl ProductList for Fixed {
    fn products(&self) -> Result<Vec<Product>, String> {
        Ok(self.0.clone())
    }
}

fn product(code: &str, name: &str) -> Product {
    let installs = vec![Install::new(code, Some(name.into()))];
    Product {
        codes: vec![code.into()],
        installs,
        name: Some(name.into()),
        icon: None,
        key_art: None,
        background: None,
        logo: None,
    }
}

fn world() -> (Arc<FakeUpstream>, Arc<Tick>, BlizzardProducts) {
    let up = Arc::new(FakeUpstream::default());
    let cdns = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0\neu|tpr/s1live|eu.cdn.blizzard.com\n";
    up.route(PATCH_HOST, "/s1/cdns", FakeResponse::ok(cdns.as_bytes(), &[]));
    let versions = "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0\n\
                    ## seqn = 1\neu|b0000000000000000000000000000001|c0000000000000000000000000000002||1|1.23.10\n";
    up.route(PATCH_HOST, "/s1/versions", FakeResponse::ok(versions.as_bytes(), &[]));
    up.route(PATCH_HOST, "/zz/cdns", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    up.route(PATCH_HOST, "/zz/versions", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let list = Arc::new(Fixed(vec![product("s1", "StarCraft"), product("zz", "Gone")]));
    let opts = ProductOptions { region: "eu".into(), ..ProductOptions::default() };
    let products = BlizzardProducts::new(list, up.clone(), clock.clone(), opts);
    (up, clock, products)
}

fn settled(p: &BlizzardProducts) -> Vec<CatalogProduct> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let list = p.list().unwrap();
        if list.iter().all(|p| p.known) || Instant::now() > until {
            return list;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn products_are_listed_at_once_and_looked_up_in_the_background() {
    let (up, clock, p) = world();
    let first = p.list().unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!((first[0].code.as_str(), first[0].name.as_deref()), ("s1", Some("StarCraft")));
    let list = settled(&p);
    assert_eq!(
        (list[0].game.as_deref(), list[0].version.as_deref(), list[0].known),
        (Some("s1live"), Some("1.23.10"), true)
    );
    assert_eq!((list[1].game.as_deref(), list[1].version.as_deref(), list[1].known), (None, None, true));
    assert_eq!(p.code_of("s1live").as_deref(), Some("s1"));
    assert_eq!(p.code_of("nothing"), None);
    // Kept: no more lookups until they are due.
    let calls = up.calls();
    let _ = p.list();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(up.calls(), calls);
    clock.0.fetch_add(11 * 60_000, Ordering::SeqCst);
    let _ = p.list();
    let until = Instant::now() + Duration::from_secs(5);
    while up.calls() < calls + 4 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(up.calls(), calls + 4, "both products looked up again");
}

#[test]
fn builds_find_a_game_not_in_the_stash_through_its_directory() {
    let (up, clock, p) = world();
    let p = Arc::new(p);
    let _ = settled(&p);
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let opts = BuildOptions { region: "eu".into(), ..BuildOptions::default() };
    let builds = BlizzardBuilds::new(up.clone(), Arc::new(store), clock.clone(), opts).with_products(p);
    let c = builds.check("s1live");
    assert_eq!((c.product.as_deref(), c.version().map(|v| v.name.as_str())), (Some("s1"), Some("1.23.10")));
}

#[test]
fn asking_for_an_unknown_directory_starts_the_lookups() {
    let (_, _, p) = world();
    assert_eq!(p.code_of("s1live"), None, "nothing looked up yet");
    let until = Instant::now() + Duration::from_secs(5);
    while p.code_of("s1live").is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(p.code_of("s1live").as_deref(), Some("s1"));
}

#[test]
fn a_game_under_another_cdn_root_has_its_directory() {
    let up = Arc::new(FakeUpstream::default());
    let cdns = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0\neu|cortez/Cerberus-B-Live|eu.cdn.blizzard.com\n";
    up.route(PATCH_HOST, "/btlr/cdns", FakeResponse::ok(cdns.as_bytes(), &[]));
    up.route(PATCH_HOST, "/btlr/versions", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let list = Arc::new(Fixed(vec![product("btlr", "Call of Duty: Black Ops 6")]));
    let opts = ProductOptions { region: "eu".into(), ..ProductOptions::default() };
    let p = BlizzardProducts::new(list, up, clock, opts);
    assert_eq!(settled(&p)[0].game.as_deref(), Some("Cerberus-B-Live"));
    assert_eq!(p.code_of("Cerberus-B-Live").as_deref(), Some("btlr"));
}

#[test]
fn products_sharing_a_directory_are_its_flavours() {
    let up = Arc::new(FakeUpstream::default());
    let table = |v: &str, build: &str| {
        format!(
            "Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!String:0\n\
             ## seqn = 1\neu|{build}|c0000000000000000000000000000002||1|{v}\n"
        )
    };
    let cdns = "Name!STRING:0|Path!STRING:0|Hosts!STRING:0\neu|tpr/wow|eu.cdn.blizzard.com\n";
    for code in ["wow", "wow_classic"] {
        up.route(PATCH_HOST, &format!("/{code}/cdns"), FakeResponse::ok(cdns.as_bytes(), &[]));
    }
    up.route(
        PATCH_HOST,
        "/wow/versions",
        FakeResponse::ok(table("12.1.5", "b0000000000000000000000000000001").as_bytes(), &[]),
    );
    up.route(
        PATCH_HOST,
        "/wow_classic/versions",
        FakeResponse::ok(table("5.5.4", "b0000000000000000000000000000003").as_bytes(), &[]),
    );
    let wow = Product {
        codes: vec!["wow".into(), "wow_classic".into()],
        installs: vec![
            Install::new("wow", Some("World of Warcraft".into())),
            Install::new("wow_classic", Some("World of Warcraft Classic".into())),
        ],
        name: Some("World of Warcraft".into()),
        icon: None,
        key_art: None,
        background: None,
        logo: None,
    };
    let clock = Arc::new(Tick(AtomicU64::new(1_000)));
    let opts = ProductOptions { region: "eu".into(), ..ProductOptions::default() };
    let p = Arc::new(BlizzardProducts::new(Arc::new(Fixed(vec![wow])), up.clone(), clock.clone(), opts));
    let listed = settled(&p);
    assert_eq!(
        listed.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(),
        vec!["wow", "wow_classic"],
        "one entry per install"
    );
    assert!(
        listed.iter().all(|c| c.family.as_deref() == Some("World of Warcraft") && c.game.as_deref() == Some("wow"))
    );
    assert_eq!(p.codes_of("wow"), vec!["wow", "wow_classic"], "the main install first");
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let opts = BuildOptions { region: "eu".into(), ..BuildOptions::default() };
    let builds = BlizzardBuilds::new(up.clone(), Arc::new(store), clock, opts).with_products(p);
    let c = builds.check("wow");
    assert_eq!((c.product.as_deref(), c.version().map(|v| v.name.as_str())), (Some("wow"), Some("12.1.5")));
    let f: Vec<(&str, Option<&str>, Option<&str>)> = c
        .flavours
        .iter()
        .map(|f| (f.product.as_str(), f.name.as_deref(), f.build.as_ref().map(|b| b.version.name.as_str())))
        .collect();
    assert_eq!(f, vec![("wow_classic", Some("World of Warcraft Classic"), Some("5.5.4"))]);
    // A flavour's own code is a game the builds check (and prefill) too.
    assert_eq!(builds.check("wow_classic").version().map(|v| v.name.as_str()), Some("5.5.4"));
}
