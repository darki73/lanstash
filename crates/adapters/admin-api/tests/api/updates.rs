//! Tests for `lanstash_adapter_admin_api::api`'s auto-update path: the games kept up to date.

use lanstash_app::auto_update::{AutoUpdate, KEPT};
use lanstash_domain::window::Window;
use lanstash_ports::keep::Keep;

use super::*;

/// Followed games kept from before.
struct Kept;

impl Keep for Kept {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok((name == KEPT).then(|| b"blizzard/osi\nsteam/10\n".to_vec()))
    }
    fn save(&self, _: &str, _: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn the_followed_games_are_listed_and_the_path_is_404_when_updates_are_off() {
    let off = get(&api(), "GET", "/api/v1/auto-update");
    assert_eq!(off.status, 404);
    let window = Window::parse("00:00-24:00").unwrap();
    let auto = AutoUpdate::new(Vec::new(), Arc::new(FixedClock(0)), window).with_keep(Arc::new(Kept));
    let a = api().with_auto_update(Arc::new(auto));
    let on = get(&a, "GET", "/api/v1/auto-update");
    assert_eq!((on.status, on.body.as_str()), (200, "{\"followed\":[\"blizzard/osi\",\"steam/10\"]}\n"));
}
