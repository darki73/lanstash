//! Tests for `api::hidden`: the games kept private, listed, marked and shown again.

use std::collections::BTreeMap;

use lanstash_app::hidden::HiddenGames;
use lanstash_ports::keep::Keep;

use super::*;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl Keep for Memory {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.0.lock().unwrap().get(name).cloned())
    }
    fn save(&self, name: &str, data: &[u8]) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.into(), data.to_vec());
        Ok(())
    }
}

#[test]
fn games_are_marked_hidden_listed_and_shown_again() {
    let a = api().with_hidden(Arc::new(HiddenGames::new(Arc::new(Memory::default())).unwrap()));
    assert_eq!(get(&a, "GET", "/api/v1/hidden").body, "{\"hidden\":[]}\n");
    let put = get(&a, "PUT", "/api/v1/hidden/steam/123");
    assert_eq!((put.status, put.body.as_str()), (200, "{\"hidden\":[\"steam/123\"]}\n"));
    get(&a, "PUT", "/api/v1/hidden/blizzard/wow");
    assert_eq!(get(&a, "GET", "/api/v1/hidden").body, "{\"hidden\":[\"blizzard/wow\",\"steam/123\"]}\n");
    assert_eq!(get(&a, "DELETE", "/api/v1/hidden/steam/123").body, "{\"hidden\":[\"blizzard/wow\"]}\n");
}

#[test]
fn a_path_that_names_no_game_is_refused_and_without_the_list_it_is_404() {
    let a = api().with_hidden(Arc::new(HiddenGames::new(Arc::new(Memory::default())).unwrap()));
    assert_eq!(get(&a, "PUT", "/api/v1/hidden/steam").status, 400);
    assert_eq!(get(&a, "PUT", "/api/v1/hidden/steam/a%20b").status, 400);
    assert_eq!(get(&a, "POST", "/api/v1/hidden/steam/1").status, 405);
    assert_eq!(get(&api(), "GET", "/api/v1/hidden").status, 404);
}
