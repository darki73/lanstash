//! Battle.net's catalog: built through the upstream on first use, refreshed
//! when due, and asked for art and names by CDN path. The documents are
//! hand-made, shaped like Battle.net's.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lanstash_app::art::{Art, ArtOptions};
use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_battlenet_app::catalog::{BlizzardCatalog, CatalogOptions, CodeOf};
use lanstash_battlenet_domain::tact::PATCH_HOST;
use lanstash_ports::art::{ArtError, ArtKind};
use lanstash_ports::clock::Clock;
use lanstash_ports::names::GameNames;
use lanstash_ports::upstream::Source;

#[derive(Default)]
struct Tick(AtomicU64);
impl Clock for Tick {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

const CDN: &str = "us.cdn.test";
const BUILD: &str = "aaaa0000000000000000000000000001";
const ROOT: &str = "bbbb0000000000000000000000000002";
const HERO: &str = "cccc0000000000000000000000000003";
const OSI: &str = "dddd0000000000000000000000000004";
const HERO_ICON: &str = "eeee0000000000000000000000000005";
const HERO_ART: &str = "ffff0000000000000000000000000006";
const OSI_ICON: &str = "abab0000000000000000000000000007";

fn data(hash: &str) -> String {
    format!("/tpr/catalogs/data/{}/{}/{hash}", &hash[..2], &hash[2..4])
}

fn text(s: &str) -> FakeResponse {
    FakeResponse::ok(s.as_bytes(), &[("Content-Type", "binary/octet-stream")])
}

fn versions(build: &str) -> FakeResponse {
    text(&format!(
        "Region!STRING:0|BuildConfig!HEX:16|BuildId!DEC:4|VersionsName!String:0\n## seqn = 1\nPUB-30|{build}|407|5240\n"
    ))
}

fn fragment(install: &str, code: &str, stem: &str, icon: &str, art: Option<&str>) -> FakeResponse {
    let art_file = art.map_or(String::new(), |h| format!(r#","{stem}_KEY_ART":{{"hash":"{h}"}}"#));
    let art_key = art.map_or(String::new(), |_| format!(r#","key_art":"{stem}_KEY_ART""#));
    text(&format!(
        r#"{{"installs":{{"{install}":{{"tact_product":"{code}"}}}},
        "products":[{{"base":{{"name":"{stem}_NAME","icon_medium":"{stem}_ICON_MEDIUM"{art_key}}}}}],
        "strings":{{"default":{{"{stem}_NAME":"Name of {code}"}}}},
        "files":{{"default":{{"{stem}_ICON_MASSIVE":{{"hash":"{icon}"}}{art_file}}}}}}}"#
    ))
}

/// A catalog with Heroes of the Storm (code `hero`, path `Hero-Live-a`) and
/// Diablo II (code and path `osi`, no key art).
fn battle_net() -> Arc<FakeUpstream> {
    let up = Arc::new(FakeUpstream::default());
    up.route(PATCH_HOST, "/catalogs/versions", versions(BUILD));
    up.route(
        PATCH_HOST,
        "/catalogs/cdns",
        text(&format!("Name!STRING:0|Path!STRING:0|Hosts!STRING:0\nus|tpr/catalogs|{CDN} other\n")),
    );
    up.route(
        CDN,
        &format!("/tpr/catalogs/config/aa/aa/{BUILD}"),
        text(&format!("# Build Configuration\nroot = {ROOT}\n")),
    );
    up.route(
        CDN,
        &data(ROOT),
        text(&format!(r#"{{"fragments":[{{"name":"heroes","hash":"{HERO}"}},{{"name":"osi","hash":"{OSI}"}},{{"name":"x","hash":"{OSI}","requires":["cn"]}}]}}"#)),
    );
    up.route(CDN, &data(HERO), fragment("heroes", "hero", "heroes#HEROES", HERO_ICON, Some(HERO_ART)));
    up.route(CDN, &data(OSI), fragment("osi", "osi", "osi#OSI", OSI_ICON, None));
    up
}

/// The patch service's answer to which product a CDN path is (here: `Hero-Live-a` is `hero`).
struct Codes;
impl CodeOf for Codes {
    fn code_of(&self, game: &str) -> Option<String> {
        (game == "Hero-Live-a").then(|| "hero".to_owned())
    }
}

fn codes() -> Arc<dyn CodeOf> {
    Arc::new(Codes)
}

fn catalog(up: &Arc<FakeUpstream>, clock: &Arc<Tick>) -> BlizzardCatalog {
    let opts = CatalogOptions { refresh_ms: 1_000, retry_ms: 100, ..CatalogOptions::default() };
    BlizzardCatalog::new(up.clone(), clock.clone(), opts)
}

#[test]
fn finds_art_by_cdn_path_through_the_product_code() {
    let up = battle_net();
    let c = catalog(&up, &Arc::new(Tick::default()));
    let at = |hash| Ok(Source { host: CDN.into(), port: None, target: data(hash) });
    assert_eq!(c.locate("Hero-Live-a", ArtKind::Icon), Err(ArtError::Missing), "no code for the path yet");
    let held = codes();
    c.set_codes(Arc::downgrade(&held));
    assert_eq!(c.locate("Hero-Live-a", ArtKind::Icon), at(HERO_ICON), "the patch service maps the path to hero");
    assert_eq!(c.locate("Hero-Live-a", ArtKind::Header), at(HERO_ART));
    assert_eq!(c.locate("osi", ArtKind::Icon), at(OSI_ICON), "the path itself as a code");
    assert_eq!(c.locate("osi", ArtKind::Header), Err(ArtError::Missing), "no key art");
    assert_eq!(c.locate("bnt004", ArtKind::Icon), Err(ArtError::Missing));
    assert_eq!(c.locate("osi", ArtKind::Background), Err(ArtError::Missing), "no background named");
    assert_eq!(up.calls(), 6, "versions, cdns, config, root, two fragments: once");
    let patch = up.seen.lock().unwrap().iter().filter(|r| r.host == PATCH_HOST).count();
    assert_eq!(patch, 2);
    assert_eq!(c.name("blizzard", "Hero-Live-a").as_deref(), Some("Name of hero"));
    assert_eq!(c.name("steam", "Hero-Live-a"), None);
}

#[test]
fn checks_again_when_due_and_rebuilds_only_for_a_new_build() {
    let up = battle_net();
    let clock = Arc::new(Tick::default());
    let c = catalog(&up, &clock);
    let first = c.locate("osi", ArtKind::Icon);
    assert!(first.is_ok(), "{first:?}");
    clock.0.store(1_000, Ordering::SeqCst);
    assert!(c.locate("osi", ArtKind::Icon).is_ok());
    assert_eq!(up.calls(), 7, "only the versions again");
    // A new build that fails part way keeps the old index, and is retried sooner.
    up.route(PATCH_HOST, "/catalogs/versions", versions("9999aaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    clock.0.store(2_000, Ordering::SeqCst);
    assert!(c.locate("osi", ArtKind::Icon).is_ok(), "old index kept");
    let calls = up.calls();
    clock.0.store(2_100, Ordering::SeqCst);
    assert!(c.locate("osi", ArtKind::Icon).is_ok());
    assert!(up.calls() > calls, "retried after retry_ms");
}

#[test]
fn without_a_catalog_art_is_an_upstream_failure() {
    let up = Arc::new(FakeUpstream::default());
    let c = catalog(&up, &Arc::new(Tick::default()));
    assert!(matches!(c.locate("osi", ArtKind::Icon), Err(ArtError::Upstream(e)) if e.contains("Battle.net catalog")));
    assert_eq!(c.name("blizzard", "osi"), None);
}

#[test]
fn art_serves_battle_net_images_typed_by_their_bytes() {
    let up = battle_net();
    up.route(
        CDN,
        &data(HERO_ICON),
        FakeResponse::ok(b"\x89PNG\r\n\x1a\nicon", &[("Content-Type", "binary/octet-stream")]),
    );
    let clock = Arc::new(Tick::default());
    let c = catalog(&up, &clock);
    let held = codes();
    c.set_codes(Arc::downgrade(&held));
    let a = Art::new(up.clone(), clock.clone(), ArtOptions::default()).with_source("blizzard", Arc::new(c));
    let img = a.get("blizzard", "Hero-Live-a", ArtKind::Icon).unwrap();
    assert_eq!((img.content_type, img.body.len()), ("image/png", 12));
    assert_eq!(a.get("blizzard", "osi", ArtKind::Header), Err(ArtError::Missing));
}

/// A code port that lists the catalog again while answering, as the real products do.
struct ListsAgain(Arc<BlizzardCatalog>);
impl CodeOf for ListsAgain {
    fn code_of(&self, game: &str) -> Option<String> {
        let _ = self.0.products();
        (game == "Hero-Live-a").then(|| "hero".to_owned())
    }
}

#[test]
fn a_name_does_not_wait_on_itself() {
    let up = battle_net();
    let c = Arc::new(catalog(&up, &Arc::new(Tick::default())));
    assert!(c.locate("osi", ArtKind::Icon).is_ok(), "the index is built");
    let port: Arc<dyn CodeOf> = Arc::new(ListsAgain(Arc::clone(&c)));
    c.set_codes(Arc::downgrade(&port));
    let (tx, rx) = std::sync::mpsc::channel();
    let named = Arc::clone(&c);
    std::thread::spawn(move || tx.send(named.name("blizzard", "Hero-Live-a")).unwrap());
    let got = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("the name lookup deadlocked");
    assert_eq!(got.as_deref(), Some("Name of hero"));
}
