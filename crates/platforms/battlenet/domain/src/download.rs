//! A build's download manifest: every file a client may download, with its
//! encoded key and size, and tags (platform, architecture, locale, region,
//! ...) that say which installs want it. Its encoded key is the second hash of
//! the build config's `download` entry; the file is BLTE ([`crate::blte`]).
//!
//! Layout after BLTE (all big-endian): `"DL"`, version (u8), key size (u8),
//! has checksum (u8), entry count (u32), tag count (u16); version 2 adds a flag
//! size (u8), version 3 a base priority (i8) and 3 bytes. Then the entries:
//! key, size (u40), priority (i8), a checksum (u32) when present, flags; then
//! the tags: a NUL-terminated name, a type (u16) and a bitmap of the entries,
//! most significant bit first.
//!
//! What an install takes ([`select`]): for each tag type, an entry must carry
//! one of the chosen tags of that type; a type none of whose tags is chosen
//! does not filter. Another install ([`retarget`]: another platform, locale)
//! swaps the chosen tags of the kinds it names; [`install_tags`] lists the
//! platforms, architectures and locales a build has installs for.

use alloc::string::String;
use alloc::vec::Vec;

/// One file of the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadEntry {
    /// The encoded key, lower-case hex.
    pub key: String,
    /// Its encoded size: the bytes a client downloads.
    pub size: u64,
}

/// One tag and the entries carrying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub name: String,
    /// What the tag is about (platform, locale, ...); numbers differ between products.
    pub kind: u16,
    mask: Vec<u8>,
}

impl Tag {
    /// Whether entry `i` carries the tag.
    #[must_use]
    pub fn has(&self, i: usize) -> bool {
        self.mask.get(i >> 3).is_some_and(|b| b >> (7 - (i & 7)) & 1 == 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadManifest {
    pub entries: Vec<DownloadEntry>,
    pub tags: Vec<Tag>,
}

/// Why a manifest could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestError {
    /// Not `DL`, an unknown version, or a key size of 0.
    BadHeader,
    /// The data ends before the entries or tags it announces.
    Truncated,
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, &x| (a << 8) | u64::from(x))
}

/// Reads a decoded download manifest.
///
/// # Errors
/// [`ManifestError`].
pub fn parse(data: &[u8]) -> Result<DownloadManifest, ManifestError> {
    let h = data.get(..11).ok_or(ManifestError::Truncated)?;
    let (version, key_size, checksum) = (h[2], usize::from(h[3]), h[4] != 0);
    if &h[..2] != b"DL" || !(1..=3).contains(&version) || key_size == 0 {
        return Err(ManifestError::BadHeader);
    }
    let count = be(&h[5..9]) as usize;
    let tag_count = be(&h[9..11]) as usize;
    let mut pos = 11;
    let mut flags = 0;
    if version >= 2 {
        flags = usize::from(*data.get(pos).ok_or(ManifestError::Truncated)?);
        pos += 1;
    }
    if version >= 3 {
        pos += 4;
    }
    let entry = key_size + 5 + 1 + if checksum { 4 } else { 0 } + flags;
    let table = data.get(pos..pos + count.checked_mul(entry).ok_or(ManifestError::Truncated)?);
    let table = table.ok_or(ManifestError::Truncated)?;
    let entries = table
        .chunks_exact(entry)
        .map(|e| DownloadEntry {
            key: e[..key_size].iter().map(|b| alloc::format!("{b:02x}")).collect(),
            size: be(&e[key_size..key_size + 5]),
        })
        .collect();
    pos += table.len();
    let mask_len = count.div_ceil(8);
    let mut tags = Vec::with_capacity(tag_count.min(1024));
    for _ in 0..tag_count {
        let rest = data.get(pos..).ok_or(ManifestError::Truncated)?;
        let end = rest.iter().position(|&b| b == 0).ok_or(ManifestError::Truncated)?;
        let name = String::from_utf8_lossy(&rest[..end]).into_owned();
        let t = rest.get(end + 1..end + 3 + mask_len).ok_or(ManifestError::Truncated)?;
        tags.push(Tag { name, kind: be(&t[..2]) as u16, mask: t[2..].to_vec() });
        pos += end + 3 + mask_len;
    }
    Ok(DownloadManifest { entries, tags })
}

/// The entries an install with the `chosen` tags takes (names compared
/// without case), by index.
#[must_use]
pub fn select(m: &DownloadManifest, chosen: &[&str]) -> Vec<usize> {
    let picked = |t: &&Tag| chosen.iter().any(|c| c.eq_ignore_ascii_case(&t.name));
    let mut kinds: Vec<u16> = m.tags.iter().filter(picked).map(|t| t.kind).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let by_kind: Vec<Vec<&Tag>> =
        kinds.iter().map(|k| m.tags.iter().filter(picked).filter(|t| t.kind == *k).collect()).collect();
    (0..m.entries.len()).filter(|&i| by_kind.iter().all(|ts| ts.iter().any(|t| t.has(i)))).collect()
}

/// The tag of `m` called `name` (without case).
fn tag<'a>(m: &'a DownloadManifest, name: &str) -> Option<&'a Tag> {
    m.tags.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

/// The `chosen` tags of an install, made another's: each tag of `install` takes the place of the
/// chosen ones of its kind (`OSX` for `Windows`, `arm64` for `x86_64`); the locale, region and the
/// rest stay.
///
/// # Errors
/// A tag of `install` the manifest does not have.
pub fn retarget(m: &DownloadManifest, chosen: &[String], install: &[&str]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = chosen.to_vec();
    for name in install {
        let t = tag(m, name).ok_or_else(|| alloc::format!("this build has no install tag {name}"))?;
        out.retain(|c| tag(m, c).is_none_or(|ct| ct.kind != t.kind));
        out.push(t.name.clone());
    }
    Ok(out)
}

/// The platforms the Battle.net app installs games on: a manifest also names consoles, phones and
/// servers, whose installs never come from Battle.net's CDN to a PC.
pub const APP_PLATFORMS: &[&str] = &["Windows", "OSX"];

/// What a build has installs for, by the manifest's tags: an install may name any of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installs {
    /// Of the kind of `Windows`, those the app installs on ([`APP_PLATFORMS`]).
    pub platforms: Vec<String>,
    /// Of the kind of `x86_64`.
    pub architectures: Vec<String>,
    /// Of the kind of `enUS`.
    pub locales: Vec<String>,
}

/// What `m` has installs for (kinds are numbered differently from one product to another, so
/// each is found by a tag every product names).
#[must_use]
pub fn install_tags(m: &DownloadManifest) -> Installs {
    let of = |known: &str| -> Vec<String> {
        tag(m, known)
            .map_or_else(Vec::new, |k| m.tags.iter().filter(|t| t.kind == k.kind).map(|t| t.name.clone()).collect())
    };
    let app = |p: &String| APP_PLATFORMS.iter().any(|a| a.eq_ignore_ascii_case(p));
    Installs {
        platforms: of("Windows").into_iter().filter(app).collect(),
        architectures: of("x86_64"),
        locales: of("enUS"),
    }
}
