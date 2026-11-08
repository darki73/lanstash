//! Tests for `lanstash_riot::profile`.

use lanstash_domain::service::{FetchMode, Profile, Route};
use lanstash_riot::profile::Riot;

#[test]
fn caches_all_but_listings_and_versions() {
    let r = Riot;
    assert_eq!(r.service(), "riot");
    assert_eq!(r.route("/channels/public/releaselisting_EUW.json"), Route::Pass);
    assert_eq!(r.route("/lol/game.version"), Route::Pass);
    assert_eq!(r.route("/bundles/A1.bundle"), Route::Object { game: "_".into(), path: "/bundles/A1.bundle" });
    assert_eq!(r.fetch_mode(), FetchMode::Ranges);
}
