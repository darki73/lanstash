//! Tests for `lanstash_domain::service`.

mod hosts;

use std::sync::Arc;

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::segment::header::Service;
use lanstash_domain::service::*;

fn obj<'a>(game: &str, path: &'a str) -> Route<'a> {
    Route::Object { game: game.into(), path }
}

/// A profile that caches nothing.
#[derive(Debug)]
struct Nothing;

impl Profile for Nothing {
    fn service(&self) -> &str {
        "nothing"
    }
    fn route<'a>(&self, _target: &'a str) -> Route<'a> {
        Route::Pass
    }
    fn fetch_mode(&self) -> FetchMode {
        FetchMode::WholeObject
    }
    fn body_check(&self, _head: &ResponseHeadIn) -> Option<BodyCheck> {
        None
    }
}

#[test]
fn generic_caches_all_but_listings_and_versions() {
    let g = Generic { name: "riot".into() };
    assert_eq!(g.service(), "riot");
    assert_eq!(g.route("/channels/public/releaselisting_EUW.json"), Route::Pass);
    assert_eq!(g.route("/bundles/A1.bundle?x"), obj("_", "/bundles/A1.bundle"));
    assert_eq!(g.route("bundles"), Route::Pass, "not a path");
    assert_eq!(route_all("/a/b/c", |p| component(p, 1)), obj("b", "/a/b/c"));
    assert_eq!(route_all("/a", |_| Some("")), obj("_", "/a"), "an empty game is unknown");
}

#[test]
fn registered_profiles_win_and_others_are_generic() {
    let p = Profiles::new(vec![Arc::new(Nothing)]);
    assert_eq!(p.get("nothing").fetch_mode(), FetchMode::WholeObject);
    let other = p.get("wsus");
    assert_eq!(other.service(), "wsus");
    assert_eq!(other.route("/x"), obj("_", "/x"));
    assert_eq!(Profiles::default().get("nothing").route("/x"), obj("_", "/x"));
}

#[test]
fn codes() {
    assert_eq!(service_code("steam"), Service::STEAM);
    assert_eq!(service_code("blizzard"), Service::BLIZZARD);
    assert_eq!(service_code("epicgames"), Service::EPIC);
    assert_eq!(service_code("riot"), Service::RIOT);
    assert_eq!(service_code("example"), Service::UNKNOWN);
    assert!(uncacheable_path("/lol/game.version"));
    assert!(!uncacheable_path("/Builds/x/ChunksV4/12/AB_CD.chunk"));
}
