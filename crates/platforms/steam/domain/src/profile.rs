//! Steam's caching rules.
//!
//! Chunk URLs look like `/depot/<depot id>/chunk/<40 hex>`. The chunk id is the
//! SHA-1 of the chunk's decrypted, decompressed content (it does **not** match the
//! SHA-1 of the bytes served), so chunks are content-addressed and never go
//! stale. The served bytes are checked with `x-content-crc` (CRC-32/IEEE of the
//! whole chunk).
//!
//! Patch URLs look like `/depot/<depot id>/patch/<from manifest>/<to manifest>`:
//! the binary patch an update from one manifest to another fetches first (about
//! 128 MiB each), the same for every client
//! updating between those two manifests, so it is cached too, in its depot's
//! directory, without a checksum (Steam sends none known for it).
//!
//! Chunks and patches are cached; manifests and everything else pass through.

use alloc::format;
use alloc::string::{String, ToString};

use lanstash_domain::http::ResponseHeadIn;
use lanstash_domain::object::path_of;
use lanstash_domain::service::{BodyCheck, FetchMode, Profile, Route};

/// The host Steam clients ask for content once they see a lancache (the one name in
/// lancache's Steam cache-domains; publicly it resolves to Valve's CDN). Prefill asks
/// the cache the same way: lancache refuses other CDN names as hosts it does not serve.
pub const LANCACHE_HOST: &str = "lancache.steamcontent.com";

/// The hosts Steam content is asked of (each with its subdomains). A client that found
/// the cache asks it with the CDN's own name: `*.steamcontent.com` mostly, and Valve's
/// Akamai names (`steampipe.akamaized.net`; the `-kr` and `-partner` ones serve the same
/// content elsewhere).
pub const CONTENT_HOSTS: &[&str] =
    &["steamcontent.com", "steampipe.akamaized.net", "steampipe-kr.akamaized.net", "steampipe-partner.akamaized.net"];

/// A Steam chunk reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamChunk<'a> {
    pub depot: u64,
    /// 40 lower-case hex characters.
    pub chunk: &'a str,
}

impl SteamChunk<'_> {
    /// Object key: `steam/depot/<depot>/chunk/<sha>` (lancache's key without the slice suffix).
    #[must_use]
    pub fn cache_key(&self) -> String {
        format!("steam/depot/{}/chunk/{}", self.depot, self.chunk)
    }
}

/// Parses a request path (query string allowed) into a chunk reference.
#[must_use]
pub fn chunk_from_path(target: &str) -> Option<SteamChunk<'_>> {
    let path = target.split(['?', '#']).next()?;
    parse_depot_chunk(path.strip_prefix("/depot/")?)
}

/// Parses an object key `steam/depot/<depot>/chunk/<sha>`.
#[must_use]
pub fn chunk_from_key(key: &str) -> Option<SteamChunk<'_>> {
    parse_depot_chunk(key.strip_prefix("steam/depot/")?)
}

fn parse_depot_chunk(rest: &str) -> Option<SteamChunk<'_>> {
    let (depot, rest) = rest.split_once('/')?;
    let chunk = rest.strip_prefix("chunk/")?;
    if depot.is_empty() || !depot.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if chunk.len() != 40 || !chunk.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    Some(SteamChunk { depot: depot.parse().ok()?, chunk })
}

/// The depot of a patch path (`/depot/<depot>/patch/<from>/<to>`, all decimal; query
/// string allowed).
#[must_use]
pub fn patch_depot(target: &str) -> Option<u64> {
    let path = target.split(['?', '#']).next()?;
    let mut parts = path.strip_prefix("/depot/")?.split('/');
    let (depot, kind, from, to) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let number = |s: &str| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit());
    (kind == "patch" && parts.next().is_none() && number(depot) && number(from) && number(to))
        .then(|| depot.parse().ok())
        .flatten()
}

/// The Steam profile.
#[derive(Debug, Clone, Copy, Default)]
pub struct Steam;

impl Profile for Steam {
    fn service(&self) -> &str {
        "steam"
    }

    fn route<'a>(&self, target: &'a str) -> Route<'a> {
        match chunk_from_path(target).map(|c| c.depot).or_else(|| patch_depot(target)) {
            Some(depot) => Route::Object { game: depot.to_string(), path: path_of(target) },
            None => Route::Pass,
        }
    }

    fn fetch_mode(&self) -> FetchMode {
        FetchMode::WholeObject
    }

    fn body_check(&self, head: &ResponseHeadIn) -> Option<BodyCheck> {
        head.header("x-content-crc").and_then(|v| v.trim().parse().ok()).map(BodyCheck::Crc32)
    }
}
