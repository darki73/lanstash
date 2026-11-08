//! Riot's caching rules: everything but release listings and version files,
//! under no particular game (its paths are not known yet).

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Route, route_all};

/// The Riot profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct Riot;

impl Profile for Riot {
    fn service(&self) -> &str {
        "riot"
    }
    fn route<'a>(&self, target: &'a str) -> Route<'a> {
        route_all(target, |_| None)
    }
    fn fetch_mode(&self) -> FetchMode {
        FetchMode::Ranges
    }
    fn body_check(&self, _head: &ResponseHeadIn) -> Option<BodyCheck> {
        None
    }
}
