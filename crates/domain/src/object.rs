//! Object identity and storage names.
//!
//! An object is one CDN file. Its key is `<service><path>` (path without the
//! query string), the same as lancache's `$cacheidentifier$uri`, and does not
//! include the host. Storage names derived from CDN paths are restricted to
//! `[A-Za-z0-9._-]` so a path can never escape its directory.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::checksum::md5::md5_hex;

/// Longest stored path component; longer ones are replaced by their hash.
pub const MAX_COMPONENT: usize = 200;
/// Deepest stored path; deeper paths are stored under their hash.
pub const MAX_DEPTH: usize = 32;
/// Game directory for objects without a known game.
pub const UNKNOWN_GAME: &str = "_";

/// Request path without query string or fragment.
#[must_use]
pub fn path_of(target: &str) -> &str {
    target.split(['?', '#']).next().unwrap_or(target)
}

/// Whether `path` names one file one way only: no `.` or `..` segment, no encoded dot,
/// slash or backslash, no backslash. An origin reads `/a/../b` as `/b`; kept as a key of
/// its own, every such spelling would be fetched and stored again.
#[must_use]
pub fn plain_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    !path.contains('\\')
        && !["%2e", "%2f", "%5c"].iter().any(|e| lower.contains(e))
        && !path.split('/').any(|seg| seg == "." || seg == "..")
}

/// The object key: `<service><path>`, e.g. `blizzard/tpr/hs/data/8e/fc/8efc…`.
#[must_use]
pub fn object_key(service: &str, path: &str) -> String {
    format!("{service}{path}")
}

fn allowed(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')
}

fn hashed(s: &str) -> String {
    let h = md5_hex(s.as_bytes());
    let mut out = String::from("_h_");
    out.extend(h.iter().map(|&c| char::from(c)));
    out
}

/// One safe path component: allowed bytes kept, others percent-encoded.
/// Empty, `.`, `..` and over-long results become `_h_<md5>`.
#[must_use]
pub fn safe_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if allowed(b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    if out.is_empty() || out == "." || out == ".." || out.len() > MAX_COMPONENT {
        return hashed(s);
    }
    out
}

/// Directory name for a game id (as [`safe_component`]; empty → [`UNKNOWN_GAME`]).
#[must_use]
pub fn game_dir(game: &str) -> String {
    if game.is_empty() { String::from(UNKNOWN_GAME) } else { safe_component(game) }
}

/// Relative file path for an object path: `/tpr/hs/data/8e/fc/x` → `tpr/hs/data/8e/fc/x`,
/// every component made safe. Paths with too many components are stored under their hash.
#[must_use]
pub fn safe_rel_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() || parts.len() > MAX_DEPTH {
        return hashed(path);
    }
    let safe: Vec<String> = parts.iter().map(|p| safe_component(p)).collect();
    safe.join("/")
}

/// An alternative name for an object file whose normal name is taken by another
/// object (e.g. on a case-insensitive file system).
#[must_use]
pub fn fallback_rel_path(key: &str) -> String {
    hashed(key)
}
