//! Tests for `lanstash_epic::profile`.

use lanstash_domain::service::{FetchMode, Profile, Route};
use lanstash_epic::profile::Epic;

fn obj<'a>(game: &str, path: &'a str) -> Route<'a> {
    Route::Object { game: game.into(), path }
}

#[test]
fn games_come_from_the_build_path() {
    let e = Epic;
    assert_eq!(e.service(), "epicgames");
    assert_eq!(
        e.route("/Builds/Org/o-1/appname/default/ChunksV4/12/AB.chunk"),
        obj("appname", "/Builds/Org/o-1/appname/default/ChunksV4/12/AB.chunk")
    );
    assert_eq!(
        e.route("/Builds/Fortnite/CloudDir/ChunksV4/1/A.chunk"),
        obj("Fortnite", "/Builds/Fortnite/CloudDir/ChunksV4/1/A.chunk")
    );
    assert_eq!(e.route("/x.chunk"), obj("_", "/x.chunk"));
    assert_eq!(e.route("/Builds/x/y.version"), Route::Pass);
    assert_eq!(e.fetch_mode(), FetchMode::Ranges);
}

#[test]
fn the_hosts_lancaches_list_lacks_are_named() {
    assert_eq!(lanstash_epic::profile::CONTENT_HOSTS, &["google.epicgamescdn.com"]);
}
