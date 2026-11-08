//! Tests for `lanstash_steam_domain::names`.

use lanstash_domain::names::NameTable;
use lanstash_steam_domain::names::DEPOT_WINDOW;

#[test]
fn depots_take_their_app_name() {
    let mut t = NameTable::default();
    t.group_numbered("steam", DEPOT_WINDOW);
    t.insert("steam", "1422450", "Deadlock");
    assert_eq!(t.get("steam", "1422456"), Some("Deadlock"));
    assert_eq!(t.get("steam", &(1_422_450 + DEPOT_WINDOW + 1).to_string()), None);
}
