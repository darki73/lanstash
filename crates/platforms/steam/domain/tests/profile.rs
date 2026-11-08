//! Tests for `lanstash_steam_domain::profile`.

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Route};
use lanstash_steam_domain::profile::*;

const SHA: &str = "9de1d873a59f1c1d108f63112ffe8903dc4b5ae1";

#[test]
fn parses_paths_and_keys() {
    let p = format!("/depot/2357571/chunk/{SHA}");
    let c = chunk_from_path(&p).unwrap();
    assert_eq!(c.depot, 2_357_571);
    assert_eq!(c.cache_key(), format!("steam/depot/2357571/chunk/{SHA}"));
    assert_eq!(chunk_from_key(&c.cache_key()), Some(c.clone()));
    assert_eq!(chunk_from_path(&format!("{p}?x=1")), Some(c));
}

#[test]
fn rejects_non_chunks() {
    assert!(chunk_from_path("/depot/1/manifest/123/5").is_none());
    assert!(chunk_from_path("/depot/x/chunk/abc").is_none());
    assert!(chunk_from_path(&format!("/depot/1/chunk/{}", SHA.to_uppercase())).is_none());
    assert!(chunk_from_path(&format!("/depot/1/chunk/{SHA}0")).is_none());
    assert!(chunk_from_path("/lancache-heartbeat").is_none());
}

#[test]
fn steams_content_hosts_are_valves_own() {
    use lanstash_domain::service::hosts::HostRules;
    let mut rules = HostRules::default();
    for h in CONTENT_HOSTS {
        rules.add_host("steam", h);
    }
    for host in ["steampipe.akamaized.net", "fastly.cdn.steampipe.steamcontent.com", "cache8-sgp1.steamcontent.com"] {
        assert_eq!(rules.classify(host), Some("steam"), "{host}");
    }
    for host in ["akamaized.net", "other.akamaized.net", "evilsteamcontent.com"] {
        assert_eq!(rules.classify(host), None, "{host}: not Valve's");
    }
}

#[test]
fn profile_routes_chunks_and_patches() {
    let t = format!("/depot/7/chunk/{SHA}?x=1");
    assert_eq!(Steam.route(&t), Route::Object { game: "7".into(), path: &t[..t.len() - 4] });
    assert_eq!(Steam.route("/depot/7/manifest/1/5/2"), Route::Pass);
    // A patch from one manifest to the next.
    let p = "/depot/2138330/patch/1003893636455474164/5565496786254603194";
    assert_eq!(Steam.route(p), Route::Object { game: "2138330".into(), path: p });
    assert_eq!(patch_depot(&format!("{p}?x=1")), Some(2_138_330));
    for wrong in
        ["/depot/1/patch/2", "/depot/1/patch/2/3/4", "/depot/1/patch/2/x", "/depot/x/patch/2/3", "/depot/1/patch//3"]
    {
        assert_eq!(patch_depot(wrong), None, "{wrong}");
        assert_eq!(Steam.route(wrong), Route::Pass, "{wrong}");
    }
    let head =
        ResponseHeadIn { status: 200, minor_version: 1, headers: vec![("x-content-crc".into(), " 3666761081".into())] };
    assert_eq!(Steam.body_check(&head), Some(BodyCheck::Crc32(3_666_761_081)));
    assert_eq!(Steam.fetch_mode(), FetchMode::WholeObject);
}
