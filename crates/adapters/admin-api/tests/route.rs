//! Which part of the API a request asks for.

use lanstash_adapter_admin_api::route::{GAMES, Route, is_api, rest, route, valid_game};

#[test]
fn routes_of_the_api_and_the_older_paths() {
    assert_eq!(route("GET", "/"), Route::Page);
    assert_eq!(route("GET", "/api/v1/version"), Route::Version);
    assert_eq!(route("GET", "/api/v1/status"), Route::Status);
    assert_eq!(route("HEAD", "/api/v1/status?x=1"), Route::Status);
    assert_eq!(route("GET", "/status.json"), Route::Status);
    assert_eq!(route("GET", "/api/v1/events"), Route::Events);
    assert_eq!(route("GET", "/events"), Route::Events);
    assert_eq!(route("POST", "/api/v1/shutdown"), Route::Shutdown);
    assert_eq!(route("POST", "/shutdown"), Route::Shutdown);
    assert_eq!(route("GET", "/api/v1/shutdown"), Route::Method("POST"));
    assert_eq!(route("POST", "/api/v1/status"), Route::Method("GET, HEAD"));
    assert_eq!(route("HEAD", "/api/v1/events"), Route::Method("GET"));
    assert_eq!(route("GET", "/api/v2/status"), Route::NotFound);
    assert_eq!(route("GET", "/api/v1/status/"), Route::NotFound);
    assert_eq!(route("GET", "/api/v1/art/steam/1422450"), Route::Art);
    assert_eq!(route("HEAD", "/api/v1/art/x"), Route::Art);
    assert_eq!(route("POST", "/api/v1/art/steam/1"), Route::Method("GET, HEAD"));
}

#[test]
fn platform_paths_route_by_prefix() {
    assert_eq!(route("GET", "/api/v1/builds/blizzard"), Route::Builds);
    assert_eq!(route("GET", "/api/v1/catalog/blizzard"), Route::Catalog);
    assert_eq!(route("GET", "/api/v1/games/blizzard/osi"), Route::Game);
    assert_eq!(route("GET", "/api/v1/games/blizzard/osi/nginx"), Route::Game);
    assert_eq!(route("POST", "/api/v1/games/blizzard/osi/prune"), Route::GamePrune);
    assert_eq!(route("POST", "/api/v1/games/blizzard/osi"), Route::GamePrune, "its handler answers 405 without /prune");
    assert_eq!(route("DELETE", "/api/v1/games/blizzard/sc1live"), Route::GameDelete);
    assert_eq!(route("DELETE", "/api/v1/games/steam/570"), Route::GameDelete);
    assert_eq!(route("GET", "/api/v1/prefill"), Route::PrefillStatus);
    assert_eq!(route("POST", "/api/v1/prefill/stop"), Route::PrefillStop);
    assert_eq!(route("POST", "/api/v1/prefill/blizzard/osi"), Route::PrefillStart);
    assert_eq!(route("POST", "/api/v1/prefill/blizzard/osi?build=pre"), Route::PrefillStart);
    assert_eq!(route("GET", "/api/v1/prefill/blizzard/osi"), Route::Method("POST, DELETE"));
    assert_eq!(route("DELETE", "/api/v1/prefill/blizzard/osi"), Route::PrefillUnqueue);
    assert_eq!(route("GET", "/api/v1/builds"), Route::NotFound, "no platform");
    assert_eq!(route("GET", "/api/v1/setup/epicgames/windows"), Route::Setup);
    assert_eq!(route("POST", "/api/v1/setup/epicgames/windows"), Route::Method("GET, HEAD"));
}

#[test]
fn paths_split_into_their_parts() {
    assert_eq!(rest("/api/v1/games/blizzard/osi?x=1", GAMES), "blizzard/osi");
    assert_eq!(rest("/elsewhere", GAMES), "");
    assert!(is_api("/api/v1/x"));
    assert!(!is_api("/status.json"));
}

#[test]
fn a_game_is_one_safe_path_segment() {
    for ok in ["osi", "Hero-Live-a", "1422450", "a_b"] {
        assert!(valid_game(ok), "{ok}");
    }
    for bad in ["", "a/b", "a%2Fb", "a.b", "..", &"x".repeat(65)] {
        assert!(!valid_game(bad), "{bad}");
    }
}
