//! Tests for `lanstash_domain::names`.

use lanstash_domain::names::*;

const WINDOW: u32 = 100;

/// An empty table that groups numbered Steam depots, as the composition root starts one.
fn table() -> NameTable {
    let mut t = NameTable::default();
    t.group_numbered("steam", WINDOW);
    t
}

#[test]
fn entries_by_service_and_game() {
    let mut t = table();
    t.insert("epicgames", "CrabEA", "Fortnite");
    assert_eq!(t.get("epicgames", "CrabEA"), Some("Fortnite"));
    assert_eq!(t.get("epicgames", "nope"), None);
    assert_eq!(t.len(), 1);
    assert!(!t.is_empty() && NameTable::default().is_empty());
}

#[test]
fn file_lines_and_numbered_groups() {
    let mut t = table();
    t.add_text("# games\nsteam/1422450 = Deadlock\n  blizzard/hs = Hearthstone (EU)  # override\nepicgames/CrabEA = Fortnite\n").unwrap();
    assert_eq!(t.get("blizzard", "hs"), Some("Hearthstone (EU)"));
    assert_eq!(t.get("blizzard", "osi"), None, "Battle.net names come from the catalog");
    assert_eq!(t.get("epicgames", "CrabEA"), Some("Fortnite"));
    // Depots take the nearest app id below them, within the window.
    assert_eq!(t.get("steam", "1422450"), Some("Deadlock"));
    assert_eq!(t.get("steam", "1422456"), Some("Deadlock"));
    assert_eq!(t.get("steam", "1422449"), None);
    assert_eq!(t.get("steam", &(1_422_450 + WINDOW + 1).to_string()), None);
    // A nearer app wins; a depot with its own entry is named by it.
    t.add_text("steam/1422455 = Deadlock Tools\nsteam/1422452 = Deadlock (shared)\n").unwrap();
    assert_eq!(t.get("steam", "1422456"), Some("Deadlock Tools"));
    assert_eq!(t.get("steam", "1422452"), Some("Deadlock (shared)"));
    assert_eq!(t.get("steam", "1422453"), Some("Deadlock (shared)"));
    t.add_text("epicgames/1422450 = Not grouped\n").unwrap();
    assert_eq!(t.get("epicgames", "1422456"), None, "only numbered services are grouped");
}

#[test]
fn malformed_lines_are_reported() {
    let mut t = NameTable::default();
    assert_eq!(t.add_text("steam/1 = A\nsteam 2 = B\n").unwrap_err().0, 2);
    assert_eq!(t.add_text("steam/ = A\n").unwrap_err().0, 1);
    assert_eq!(t.add_text("steam/3 =\n").unwrap_err().0, 1);
}

#[test]
fn group_is_the_key_of_the_entry_that_named_it() {
    let mut t = table();
    t.add_text("steam/1422450 = Deadlock\nblizzard/osi = D2R\n").unwrap();
    assert_eq!(t.group("steam", "1422456"), Some("1422450"), "a depot groups under its app");
    assert_eq!(t.group("steam", "1422450"), Some("1422450"));
    assert_eq!(t.group("steam", "7"), None);
    assert_eq!(t.group("blizzard", "osi"), Some("osi"));
    assert_eq!(t.group("blizzard", "bnt004"), None);
}
