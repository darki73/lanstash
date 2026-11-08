//! Tests for `lanstash_domain::private`.

use lanstash_domain::private::{Marks, masked_for};

fn marks(of: &[(&str, &[&str])]) -> Marks {
    of.iter().map(|(g, by)| ((*g).to_string(), by.iter().map(|a| (*a).to_string()).collect())).collect()
}

/// player.one is linked to steam/one, player.two to steam/two.
fn links(account: &str) -> Vec<String> {
    match account {
        "player.one" => vec!["steam/one".into()],
        "player.two" => vec!["steam/two".into()],
        _ => vec![],
    }
}

#[test]
fn a_game_only_its_marking_owner_has_is_masked_for_everyone_else() {
    let m = marks(&[("steam/1", &["player.one"])]);
    let owners = |_: &str| vec!["steam/one".to_string()];
    assert_eq!(masked_for(None, &m, &owners, &links), ["steam/1"], "signed out");
    assert_eq!(masked_for(Some("player.two"), &m, &owners, &links), ["steam/1"]);
    assert!(masked_for(Some("player.one"), &m, &owners, &links).is_empty(), "its own mark: seen");
}

#[test]
fn another_owner_who_did_not_mark_it_keeps_it_in_view() {
    let m = marks(&[("steam/1", &["player.one"])]);
    let both = |_: &str| vec!["steam/one".to_string(), "steam/two".to_string()];
    assert!(masked_for(None, &m, &both, &links).is_empty());
    let all_marked = marks(&[("steam/1", &["player.one", "player.two"])]);
    assert_eq!(masked_for(Some("viewer"), &all_marked, &both, &links), ["steam/1"], "every owner marked it");
    assert!(masked_for(Some("player.two"), &all_marked, &both, &links).is_empty());
    // An owner no account is linked to is another owner too.
    let with_stranger = |_: &str| vec!["steam/one".to_string(), "steam/three".to_string()];
    assert!(masked_for(None, &m, &with_stranger, &links).is_empty());
}

#[test]
fn a_game_without_known_owners_is_masked_by_its_mark_alone() {
    let m = marks(&[("blizzard/hs", &["player.one"]), ("steam/2", &[])]);
    let none = |_: &str| Vec::new();
    assert_eq!(masked_for(None, &m, &none, &links), ["blizzard/hs"], "a game nobody marks is not masked");
}
