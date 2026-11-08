//! Tests for `api::own`: a player's own games, by the platform accounts linked to it.

use lanstash_adapter_admin_api::api::own::own_catalog;

use super::access::{cookie_of, send, with_accounts};
use super::*;

/// A platform whose catalog has game 10 on account `someone`, and 20 on `other`.
#[derive(Debug)]
struct Owned;

impl Platform for Owned {
    fn id(&self) -> &str {
        "steam"
    }
    fn catalog(&self) -> Option<View> {
        Some(Ok(r#"{"products":[{"game":"10","name":"Mine","owners":["someone"]},{"game":"20","name":"Theirs","owners":["other"]}]}"#.into()))
    }
}

#[test]
fn a_catalog_keeps_only_the_games_the_accounts_own() {
    let catalog = r#"{"region":"x","products":[{"game":"1","owners":["a"]},{"game":"2","owners":["b"]},{"game":"3"}]}"#;
    assert_eq!(own_catalog(catalog, &["a"]), r#"{"region":"x","products":[{"game":"1","owners":["a"]}]}"#);
    assert_eq!(own_catalog("not json", &["a"]), "not json");
}

#[test]
fn a_player_sees_and_may_prefill_only_its_linked_accounts_games() {
    let a = with_accounts().with_platforms(vec![Arc::new(Owned)]);
    let admin = cookie_of(&a, "boss");
    let c = [("cookie", admin.as_str())];
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?role=player", &c, false).status, 200);
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?link=steam%2Fsomeone", &c, false).status, 200);
    assert!(send(&a, "GET", "/api/v1/users", &c, false).body.contains("\"links\":[\"steam/someone\"]"));
    let player = cookie_of(&a, "player.one");
    let p = [("cookie", player.as_str())];
    let catalog = send(&a, "GET", "/api/v1/catalog/steam", &p, false).body;
    assert!(catalog.contains("Mine") && !catalog.contains("Theirs"), "{catalog}");
    assert!(!catalog.contains("owners"), "account names stay hidden");
    assert!(send(&a, "GET", "/api/v1/session", &p, false).body.contains("\"links\":[\"steam/someone\"]"));
    // Its own game passes the check (this platform has no prefill: not found), another's is refused.
    assert_eq!(send(&a, "POST", "/api/v1/prefill/steam/10", &p, false).status, 404);
    assert_eq!(send(&a, "POST", "/api/v1/prefill/steam/20", &p, false).status, 403);
    assert_eq!(
        send(&a, "PUT", "/api/v1/users/player.one?link=steam%2Fx", &p, false).status,
        403,
        "only an admin links"
    );
}
