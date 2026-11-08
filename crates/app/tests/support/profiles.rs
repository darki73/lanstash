//! Caching rules for the core's tests, shaped like the platforms' (the core
//! never names a platform crate): `steam` caches `/depot/<n>/chunk/<x>` whole,
//! checked by `x-content-crc`; `blizzard` caches everything ranged under the
//! game below `/tpr/`.

use std::sync::Arc;

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::object::path_of;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Profiles, Route, component, route_all};

/// Whole chunks, checked by CRC-32.
#[derive(Debug)]
pub struct Chunks;

impl Profile for Chunks {
    fn service(&self) -> &str {
        "steam"
    }
    fn route<'a>(&self, target: &'a str) -> Route<'a> {
        let path = path_of(target);
        match (component(path, 0), component(path, 1), component(path, 2)) {
            (Some("depot"), Some(depot), Some("chunk")) => Route::Object { game: depot.to_owned(), path },
            _ => Route::Pass,
        }
    }
    fn fetch_mode(&self) -> FetchMode {
        FetchMode::WholeObject
    }
    fn body_check(&self, head: &ResponseHeadIn) -> Option<BodyCheck> {
        head.header("x-content-crc").and_then(|v| v.trim().parse().ok()).map(BodyCheck::Crc32)
    }
}

/// Ranged files, by the game below `/tpr/`.
#[derive(Debug)]
pub struct Ranged;

impl Profile for Ranged {
    fn service(&self) -> &str {
        "blizzard"
    }
    fn route<'a>(&self, target: &'a str) -> Route<'a> {
        route_all(target, |p| if component(p, 0) == Some("tpr") { component(p, 1) } else { None })
    }
    fn fetch_mode(&self) -> FetchMode {
        FetchMode::Ranges
    }
    fn body_check(&self, _head: &ResponseHeadIn) -> Option<BodyCheck> {
        None
    }
}

/// Both.
pub fn profiles() -> Profiles {
    Profiles::new(vec![Arc::new(Chunks), Arc::new(Ranged)])
}
