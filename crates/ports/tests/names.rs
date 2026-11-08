use lanstash_ports::names::{GameNames, NoNames};

struct One;

impl GameNames for One {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (service == "steam" && game == "7").then(|| "Seven".to_owned())
    }
}

#[test]
fn names_and_groups() {
    assert_eq!(NoNames.name("steam", "7"), None);
    assert_eq!(NoNames.group("steam", "7"), None);
    assert_eq!(One.name("steam", "7").as_deref(), Some("Seven"));
    assert_eq!(One.group("steam", "7"), None, "no groups unless a source knows them");
}
