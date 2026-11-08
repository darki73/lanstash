//! CDN services and their caching rules.
//!
//! [`hosts::HostRules`] maps a host to a service; a [`Profile`] (one per
//! platform, registered in [`Profiles`], plus [`Generic`] for any other
//! cache-domains service) decides, from the request alone, whether and where
//! an object is cached, how misses are fetched and how bodies are checked.
//! Profiles are pure: no I/O.

pub mod hosts;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::http::ResponseHeadIn;
use crate::object::{UNKNOWN_GAME, path_of};
use crate::segment::header::Service;

/// What to do with a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route<'a> {
    /// Not cached: relay to the origin.
    Pass,
    /// Cached as the object `<service><path>`, stored under `game`.
    Object { game: String, path: &'a str },
}

impl Route<'_> {
    /// An object whose path is not plain ([`crate::object::plain_path`]) is passed, not cached.
    #[must_use]
    pub fn filter_plain(self) -> Self {
        match self {
            Self::Object { path, .. } if !crate::object::plain_path(crate::object::path_of(path)) => Self::Pass,
            r => r,
        }
    }
}

/// How missing bytes are fetched from the origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchMode {
    /// The origin honours `Range`: fetch exactly the missing parts.
    Ranges,
    /// Always fetch the whole object, without `Range` (small objects, e.g. Steam chunks).
    WholeObject,
}

/// A check the whole body must pass before it is stored or completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyCheck {
    /// CRC-32/IEEE of the whole body.
    Crc32(u32),
}

/// Caching rules of one service.
pub trait Profile: Send + Sync + core::fmt::Debug {
    /// Service identifier (`steam`, `blizzard`, ...): the object key prefix and directory name.
    fn service(&self) -> &str;
    /// Whether and where `target` (path and query) is cached.
    fn route<'a>(&self, target: &'a str) -> Route<'a>;
    fn fetch_mode(&self) -> FetchMode;
    /// The check for a whole-object response, from its headers.
    fn body_check(&self, head: &ResponseHeadIn) -> Option<BodyCheck>;
}

/// Responses never cached (Riot release listings, version files).
#[must_use]
pub fn uncacheable_path(path: &str) -> bool {
    path.contains("releaselisting_") || path.ends_with(".version")
}

/// The route of a profile that caches everything except [`uncacheable_path`],
/// with the game `game` finds in the path (none: [`UNKNOWN_GAME`]).
pub fn route_all<'a>(target: &'a str, game: impl FnOnce(&str) -> Option<&str>) -> Route<'a> {
    let path = path_of(target);
    if uncacheable_path(path) || !path.starts_with('/') {
        return Route::Pass;
    }
    let game = game(path).filter(|g| !g.is_empty()).unwrap_or(UNKNOWN_GAME).to_string();
    Route::Object { game, path }
}

/// The n-th path component (0-based) of `/a/b/c`.
#[must_use]
pub fn component(path: &str, n: usize) -> Option<&str> {
    path.split('/').filter(|c| !c.is_empty()).nth(n)
}

/// Any cache-domains service without a profile of its own.
#[derive(Debug, Clone, Default)]
pub struct Generic {
    pub name: String,
}

impl Profile for Generic {
    fn service(&self) -> &str {
        &self.name
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

/// The registered profiles, by service.
#[derive(Debug, Clone, Default)]
pub struct Profiles {
    known: Vec<Arc<dyn Profile>>,
}

impl Profiles {
    #[must_use]
    pub fn new(known: Vec<Arc<dyn Profile>>) -> Self {
        Self { known }
    }

    /// The profile of `service`: its registered one, else [`Generic`].
    #[must_use]
    pub fn get(&self, service: &str) -> Arc<dyn Profile> {
        self.known
            .iter()
            .find(|p| p.service() == service)
            .cloned()
            .unwrap_or_else(|| Arc::new(Generic { name: service.to_string() }))
    }
}

/// The segment-header service code for an identifier (part of the segment format).
#[must_use]
pub fn service_code(identifier: &str) -> Service {
    match identifier {
        "steam" => Service::STEAM,
        "epicgames" => Service::EPIC,
        "blizzard" => Service::BLIZZARD,
        "riot" => Service::RIOT,
        _ => Service::UNKNOWN,
    }
}
