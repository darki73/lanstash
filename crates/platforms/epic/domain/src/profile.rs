//! Epic's caching rules: `/Builds/Org/<org>/<app>/...` or `/Builds/<app>/...`,
//! the app being the game (**hypothesis**, from URL samples only).

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Route, component, route_all};

/// Epic's CDN hosts that lancache's list lacks (each with its subdomains): the
/// launcher's own CDN list (`[Portal.CDN]` in the settings file Epic's servers
/// deliver) names `google.epicgamescdn.com` beside the listed ones. Its own
/// caches (`*.patchercache.epicgames.net`), its test host and its CDN for China
/// (Tencent) are left out.
pub const CONTENT_HOSTS: &[&str] = &["google.epicgamescdn.com"];

/// The Epic profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct Epic;

impl Profile for Epic {
    fn service(&self) -> &str {
        "epicgames"
    }
    fn route<'a>(&self, target: &'a str) -> Route<'a> {
        route_all(target, |p| match (component(p, 0), component(p, 1)) {
            (Some("Builds"), Some("Org")) => component(p, 3),
            (Some("Builds"), Some(app)) => Some(app),
            _ => None,
        })
    }
    fn fetch_mode(&self) -> FetchMode {
        FetchMode::Ranges
    }
    fn body_check(&self, _head: &ResponseHeadIn) -> Option<BodyCheck> {
        None
    }
}
