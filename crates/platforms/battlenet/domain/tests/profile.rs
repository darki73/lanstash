//! Tests for `lanstash_battlenet_domain::profile`.

use lanstash_battlenet_domain::profile::Blizzard;
use lanstash_domain::service::{FetchMode, Profile, Route};

fn obj<'a>(game: &str, path: &'a str) -> Route<'a> {
    Route::Object { game: game.into(), path }
}

#[test]
fn blizzard_games_come_from_the_product() {
    let b = Blizzard;
    assert_eq!(b.service(), "blizzard");
    assert_eq!(b.route("/tpr/hs/data/8e/fc/8efc"), obj("hs", "/tpr/hs/data/8e/fc/8efc"));
    assert_eq!(b.route("/tpr/anbs/data/00/e5/x.index?a=1"), obj("anbs", "/tpr/anbs/data/00/e5/x.index"));
    let bo6 = "/cortez/Cerberus-B-Live/data/1a/2b/1a2b";
    assert_eq!(b.route(bo6), obj("Cerberus-B-Live", bo6), "Black Ops 6 is under /cortez/");
    assert_eq!(b.route("/other/file"), obj("_", "/other/file"));
    assert_eq!(b.route("/tpr/hs/versions.version"), Route::Pass);
    assert_eq!(b.fetch_mode(), FetchMode::Ranges);
}
