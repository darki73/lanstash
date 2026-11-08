//! Game name sources.

use std::sync::Arc;

use lanstash_adapter_names::game_names::{ChainNames, TableNames};
use lanstash_domain::names::NameTable;
use lanstash_ports::names::GameNames;

struct Fixed;
impl GameNames for Fixed {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (service == "steam" && game == "7").then(|| "From login".to_owned())
    }
}

/// A starting table as the composition root makes one: numbered Steam depots, no names.
fn builtin() -> NameTable {
    let mut t = NameTable::default();
    t.group_numbered("steam", 100);
    t
}

#[test]
fn file_lines_and_chain_order() {
    let table =
        TableNames::with_text(builtin(), "names.conf", "steam/7 = From file\nsteam/1422450 = Deadlock\n").unwrap();
    assert_eq!(table.name("blizzard", "hs"), None, "no built-in names: Battle.net's come from its catalog");
    let chain = ChainNames(vec![Arc::new(Fixed), Arc::new(table)]);
    assert_eq!(chain.name("steam", "7").as_deref(), Some("From login"), "earlier sources win");
    assert_eq!(chain.name("steam", "1422456").as_deref(), Some("Deadlock"));
    assert_eq!(chain.name("riot", "x"), None);
    assert_eq!(
        TableNames::with_text(builtin(), "names.conf", "oops\n").unwrap_err(),
        "names.conf:1: expected `service/game = Name`, got \"oops\""
    );
    assert!(TableNames::new(NameTable::default()).is_empty());
}

#[test]
fn groups_come_from_the_first_source_that_knows_one() {
    let table = TableNames::with_text(builtin(), "names.conf", "steam/1422450 = Deadlock\n").unwrap();
    assert_eq!(table.group("steam", "1422456").as_deref(), Some("1422450"));
    let chain = ChainNames(vec![Arc::new(Fixed), Arc::new(table)]);
    assert_eq!(chain.group("steam", "1422453").as_deref(), Some("1422450"), "Fixed knows no groups");
    assert_eq!(chain.group("steam", "7"), None);
}
