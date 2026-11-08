//! The kinds of value a configuration key takes, read from their text: sizes, lists, paths,
//! yes-or-no, DNS servers, how often, how much to keep free, and where clients' names are asked.

use std::net::SocketAddr;
use std::path::PathBuf;

use lanstash_domain::evict::{KeepFree, parse_percent};

use super::ClientNamesFrom;

/// `system`, `off`, or DNS servers (`10.0.0.1`, `10.0.0.1:5353`, comma-separated).
pub(super) fn client_names(v: &str) -> Option<ClientNamesFrom> {
    match v.trim() {
        "system" => Some(ClientNamesFrom::System),
        "off" | "" => Some(ClientNamesFrom::Off),
        v => {
            list(v).map(dns_server).collect::<Option<Vec<_>>>().filter(|s| !s.is_empty()).map(ClientNamesFrom::Servers)
        }
    }
}

/// `off`, `weekly`, `monthly` or `<n>d` (1 to 365 days); the days, none when off.
pub(super) fn every_days(v: &str) -> Option<Option<u64>> {
    match v {
        "off" | "" => Some(None),
        "weekly" => Some(Some(7)),
        "monthly" => Some(Some(30)),
        _ => v.strip_suffix('d')?.parse().ok().filter(|n| (1..=365).contains(n)).map(Some),
    }
}

/// The entries of a comma-separated list, trimmed, empty ones left out.
pub fn list(v: &str) -> impl Iterator<Item = &str> {
    v.split(',').map(str::trim).filter(|s| !s.is_empty())
}

pub(super) fn paths(v: &str) -> Vec<PathBuf> {
    list(v).map(PathBuf::from).collect()
}

pub(super) fn boolean(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// `1.1.1.1` or `1.1.1.1:5353`.
pub(super) fn dns_server(v: &str) -> Option<SocketAddr> {
    v.parse::<SocketAddr>().ok().or_else(|| v.parse::<std::net::IpAddr>().ok().map(|ip| SocketAddr::new(ip, 53)))
}

/// `4096`, `512K`, `256M`, `1G`, `2T` (powers of 1024).
#[must_use]
pub fn size(v: &str) -> Option<u64> {
    let (num, mul) = match v.as_bytes().last()? {
        b'K' | b'k' => (&v[..v.len() - 1], 1u64 << 10),
        b'M' | b'm' => (&v[..v.len() - 1], 1 << 20),
        b'G' | b'g' => (&v[..v.len() - 1], 1 << 30),
        b'T' | b't' => (&v[..v.len() - 1], 1 << 40),
        _ => (v, 1),
    };
    num.trim().parse::<u64>().ok()?.checked_mul(mul)
}

/// `keep_free`: `10%`, a size (`500G`), or empty / `0` for no limit.
pub(super) fn keep_free(v: &str) -> Result<Option<KeepFree>, String> {
    let v = v.trim();
    if v.is_empty() || v == "0" {
        return Ok(None);
    }
    if v.ends_with('%') {
        return parse_percent(v).map(|p| Some(KeepFree::Percent(p))).ok_or(format!("`keep_free`: {v:?} is not 1%–99%"));
    }
    size(v).map(|b| Some(KeepFree::Bytes(b))).ok_or(format!("`keep_free`: invalid value {v:?}"))
}
