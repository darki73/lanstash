//! Tests for `lanstash_steam_domain::builds`.

use lanstash_domain::json::{self, Value};
use lanstash_steam_domain::builds::{AppBuild, render};

#[test]
fn the_view_has_battle_net_s_shape() {
    let apps = [
        AppBuild {
            app: 1_422_450,
            name: Some("Dead\"lock".into()),
            build: Some(26_467_182),
            have: Some(15),
            want: Some(15),
            error: None,
            lancache: Some((10, 15)),
        },
        AppBuild {
            app: 7,
            name: None,
            build: None,
            have: None,
            want: None,
            error: Some("no manifest".into()),
            lancache: None,
        },
    ];
    let text = render("windows", true, &apps);
    let v = json::parse(&text).unwrap();
    assert_eq!(v.get("region").and_then(Value::as_str), Some("windows"));
    assert!(text.contains("\"lancache\":true"));
    let games = v.get("games").unwrap().as_array();
    assert_eq!(games[0].get("game").and_then(Value::as_str), Some("1422450"));
    assert_eq!(games[0].get("version").and_then(Value::as_str), Some("26467182"));
    assert_eq!(games[0].get("name").and_then(Value::as_str), Some("Dead\"lock"));
    assert!(text.contains("\"have\":15,\"want\":15,\"plan_error\":null,\"current\":true"));
    assert!(text.contains("\"error\":null,\"lancache_share\":{\"have\":10,\"want\":15}}"));
    assert!(text.contains("\"lancache_share\":null}"));
    assert!(text.contains("\"version\":null,\"build_config\":null,\"have\":null,\"want\":null,\"plan_error\":\"no manifest\",\"current\":false"));
}

#[test]
fn current_means_all_of_a_known_install() {
    let b = |have, want| AppBuild { app: 1, name: None, build: None, have, want, error: None, lancache: None };
    assert!(b(Some(10), Some(10)).current());
    assert!(!b(Some(9), Some(10)).current());
    assert!(!b(None, Some(10)).current());
    assert!(!b(Some(0), Some(0)).current(), "an install of nothing is not current");
}
