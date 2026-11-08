//! TACT, Battle.net's content system, the parts
//! lanstash reads; all plain HTTP:
//!
//! - the patch service (`us.patch.battle.net:1119`) answers `/<code>/versions`
//!   and `/<code>/cdns` with BPSV tables: a `Name!TYPE:n|...` header, a
//!   `## seqn` line, `|`-separated rows;
//! - the CDN serves `/<path>/config/<ab>/<cd>/<hash>` (configs: `key = value`
//!   lines, values may be lists) and `/<path>/data/<ab>/<cd>/<hash>`, every
//!   file named by the MD5 of its content.
//!
//! A product's current build is its region's row of `/<code>/versions`:
//! `BuildConfig`, `CDNConfig` and `VersionsName` (the version players see).

use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The patch service: plain HTTP on a port of its own.
pub const PATCH_HOST: &str = "us.patch.battle.net";
pub const PATCH_PORT: u16 = 1119;

/// The rows of a BPSV table, each as `(column name, value)` pairs.
#[must_use]
pub fn bpsv(text: &str) -> Vec<Vec<(&str, &str)>> {
    let mut lines = text.lines().map(str::trim_end).filter(|l| !l.is_empty() && !l.starts_with("##"));
    let Some(header) = lines.next() else { return Vec::new() };
    let names: Vec<&str> = header.split('|').map(|c| c.split('!').next().unwrap_or(c)).collect();
    lines.map(|l| names.iter().copied().zip(l.split('|')).collect()).collect()
}

/// The value of column `name` in a row.
#[must_use]
pub fn column<'a>(row: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    row.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// A content hash: 32 lower-case hex digits.
#[must_use]
pub fn is_hash(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The CDN target of a file: `/<path>/<kind>/<ab>/<cd>/<hash>` (kind is
/// `config` or `data`); `None` for a malformed hash.
#[must_use]
pub fn target(path: &str, kind: &str, hash: &str) -> Option<String> {
    is_hash(hash).then(|| format!("/{path}/{kind}/{}/{}/{hash}", &hash[..2], &hash[2..4]))
}

/// The value of `key` in a build config (`key = value` lines).
#[must_use]
pub fn config_value<'a>(config: &'a str, key: &str) -> Option<&'a str> {
    config.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim())
    })
}

/// `(host, path)` to fetch a product's files from, from `/<code>/cdns`: the
/// `region` row (else the first), its first host.
#[must_use]
pub fn cdn(cdns: &str, region: &str) -> Option<(String, String)> {
    cdn_hosts(cdns, region).map(|(hosts, path)| (hosts[0].clone(), path))
}

/// Every host of the `region` row of `/<code>/cdns` (else the first row), in
/// its order, and the path; malformed hosts are left out.
#[must_use]
pub fn cdn_hosts(cdns: &str, region: &str) -> Option<(Vec<String>, String)> {
    let rows = bpsv(cdns);
    let row = rows.iter().find(|r| column(r, "Name") == Some(region)).or_else(|| rows.first())?;
    let safe = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._/".contains(&b));
    let hosts: Vec<String> =
        column(row, "Hosts")?.split_whitespace().filter(|h| safe(h) && !h.contains('/')).map(str::to_owned).collect();
    let path = column(row, "Path")?.trim_matches('/');
    (!hosts.is_empty() && safe(path) && !path.contains("..")).then(|| (hosts, path.to_owned()))
}

/// A product's current build in one region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub build_config: String,
    pub cdn_config: String,
    /// What players see (`3.3.93847`).
    pub name: String,
}

/// The `region` row of `/<code>/versions`.
#[must_use]
pub fn version(versions: &str, region: &str) -> Option<Version> {
    let rows = bpsv(versions);
    let row = rows.iter().find(|r| column(r, "Region") == Some(region))?;
    let build_config = column(row, "BuildConfig").filter(|h| is_hash(h))?.to_owned();
    let cdn_config = column(row, "CDNConfig").filter(|h| is_hash(h))?.to_owned();
    let name = column(row, "VersionsName").unwrap_or("").to_owned();
    Some(Version { build_config, cdn_config, name })
}

/// The values of a list entry of a config (`archives = h1 h2 ...`).
#[must_use]
pub fn config_list<'a>(config: &'a str, key: &str) -> Vec<&'a str> {
    config_value(config, key).map(|v| v.split_whitespace().collect()).unwrap_or_default()
}

/// The files a build config names itself (each entry of two hashes names a
/// file by its content key and its encoded key: encoding, install, download,
/// size, vfs-*, ...): `(entry, encoded key, encoded size)`, the size from the
/// entry's `-size` twin (second value) or 0. Malformed hashes are skipped.
#[must_use]
pub fn build_files(build_config: &str) -> Vec<(String, String, u64)> {
    let mut out: Vec<(String, String, u64)> = Vec::new();
    for line in build_config.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, hashes) = (key.trim(), value.split_whitespace().collect::<Vec<_>>());
        if key.ends_with("-size") || hashes.len() != 2 || !hashes.iter().all(|h| is_hash(h)) {
            continue;
        }
        let size = config_list(build_config, &format!("{key}-size")).get(1).and_then(|v| v.parse().ok());
        if !out.iter().any(|(_, k, _)| k == hashes[1]) {
            out.push((key.to_owned(), hashes[1].to_owned(), size.unwrap_or(0)));
        }
    }
    out
}

/// One entry of an archive index or a file index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// The encoded key, lower-case hex: the file's name under `data` (file
    /// index), or its place in the archive (archive index).
    pub key: String,
    pub size: u64,
    /// Offset in the archive; `None` in a file index.
    pub offset: Option<u64>,
}

/// Why an index could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexError {
    TooShort,
    /// A field of the footer is out of range.
    BadFooter,
    /// The footer counts more entries than the blocks hold.
    Truncated,
}

/// Reads a CDN index (`<hash>.index`): fixed-size blocks of entries
/// (`key_size` key bytes, a big-endian size, an optional big-endian archive
/// offset), a table of contents, and a footer ending in an 8-byte checksum:
/// `version, 2 bytes, block size in KiB, offset bytes, size bytes, key
/// bytes, checksum bytes, entry count (u32 little-endian)`. A key of all zeros ends a block's entries.
///
/// # Errors
/// [`IndexError`] when the footer or the blocks do not add up.
pub fn parse_index(data: &[u8]) -> Result<Vec<IndexEntry>, IndexError> {
    if data.len() < 20 {
        return Err(IndexError::TooShort);
    }
    // The checksum size is the second to last byte before the count.
    let checksum = usize::from(data[data.len() - 13]);
    let footer = 12 + 2 * checksum;
    if !(4..=16).contains(&checksum) || data.len() < footer {
        return Err(IndexError::BadFooter);
    }
    let f = &data[data.len() - footer + checksum..];
    let (block_kib, offset_bytes, size_bytes, key_bytes) =
        (usize::from(f[3]), usize::from(f[4]), usize::from(f[5]), usize::from(f[6]));
    let count = u32::from_le_bytes([f[8], f[9], f[10], f[11]]) as usize;
    if block_kib == 0 || !(1..=8).contains(&size_bytes) || offset_bytes > 8 || !(1..=32).contains(&key_bytes) {
        return Err(IndexError::BadFooter);
    }
    let (block, entry) = (block_kib * 1024, key_bytes + size_bytes + offset_bytes);
    let be = |b: &[u8]| b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x));
    let mut out = Vec::with_capacity(count.min(1 << 20));
    let body = &data[..data.len() - footer];
    let mut at = 0;
    while out.len() < count {
        let block_end = (at / block + 1) * block;
        if at + entry > block_end {
            at = block_end;
            continue;
        }
        let Some(e) = body.get(at..at + entry) else { return Err(IndexError::Truncated) };
        let key = &e[..key_bytes];
        if key.iter().all(|&b| b == 0) {
            at = block_end;
            continue;
        }
        out.push(IndexEntry {
            key: key.iter().map(|b| format!("{b:02x}")).collect(),
            size: be(&e[key_bytes..key_bytes + size_bytes]),
            offset: (offset_bytes > 0).then(|| be(&e[key_bytes + size_bytes..])),
        });
        at += entry;
    }
    Ok(out)
}
