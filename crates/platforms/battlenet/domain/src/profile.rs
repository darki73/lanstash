//! Battle.net's caching rules: `/tpr/<product>/{config,data,patch}/xx/yy/<hash>[.index]`,
//! content-addressed; the game is the CDN path below `/tpr/`.

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Route, component, route_all};

/// The Battle.net profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct Blizzard;

impl Profile for Blizzard {
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
