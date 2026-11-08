//! Tests for `api::private`: a player keeps its own games private; others see them masked unless
//! another owner did not mark them; admins see all.

use std::collections::BTreeMap;

use lanstash_app::private_games::PrivateGames;
use lanstash_ports::keep::Keep;

use super::access::{cookie_of, send, with_accounts};
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

/// A catalog: game 10 on account `someone`, 20 on `other`, 30 on both.
#[derive(Debug)]
struct Owned;

impl Platform for Owned {
    fn id(&self) -> &str {
        "steam"
    }
    fn catalog(&self) -> Option<View> {
        Some(Ok(r#"{"products":[{"game":"10","name":"Mine","owners":["someone"]},{"game":"20","name":"Theirs","owners":["other"]},{"game":"30","name":"Shared","owners":["someone","other"]}]}"#.into()))
    }
}

#[test]
fn a_player_keeps_its_own_games_private_and_others_see_them_masked() {
    let a = with_accounts()
        .with_platforms(vec![Arc::new(Owned)])
        .with_private(Arc::new(PrivateGames::new(Arc::new(Memory::default())).unwrap()));
    let boss = cookie_of(&a, "boss");
    let admin = [("cookie", boss.as_str())];
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?role=player", &admin, false).status, 200);
    assert_eq!(send(&a, "PUT", "/api/v1/users/player.one?link=steam%2Fsomeone", &admin, false).status, 200);
    let one = cookie_of(&a, "player.one");
    let p = [("cookie", one.as_str())];

    assert_eq!(send(&a, "GET", "/api/v1/private", &[], false).status, 401, "signed out: whose?");
    let put = send(&a, "PUT", "/api/v1/private/steam/10", &p, false);
    assert_eq!((put.status, put.body.as_str()), (200, "{\"private\":[\"steam/10\"]}\n"));
    assert_eq!(send(&a, "PUT", "/api/v1/private/steam/20", &p, false).status, 403, "not its own");
    assert_eq!(send(&a, "PUT", "/api/v1/private/steam/30", &p, false).status, 200);

    let catalog = |headers: &[(&str, &str)]| send(&a, "GET", "/api/v1/catalog/steam", headers, false).body;
    let guest = catalog(&[]);
    assert!(!guest.contains("Mine"), "only its marking owner has it: masked: {guest}");
    assert!(guest.contains("Shared"), "another owner did not mark it: in view");
    assert!(guest.contains("Theirs"));
    assert!(catalog(&p).contains("Mine"), "its own mark: seen");
    assert!(catalog(&admin).contains("Mine"), "an admin sees all");

    // An admin marks any game, for any account.
    let marked = send(&a, "PUT", "/api/v1/private/steam/20?account=player.one", &admin, false);
    assert_eq!(
        (marked.status, marked.body.as_str()),
        (200, "{\"private\":[\"steam/10\",\"steam/20\",\"steam/30\"]}\n")
    );
    let off = send(&a, "DELETE", "/api/v1/private/steam/10", &p, false);
    assert_eq!(off.body, "{\"private\":[\"steam/20\",\"steam/30\"]}\n");
    assert!(catalog(&[]).contains("Mine"), "unmarked: in view again");
    // An account's marks go with it.
    assert_eq!(send(&a, "DELETE", "/api/v1/users/player.one", &admin, false).status, 200);
    assert!(catalog(&[]).contains("Theirs"));
}
