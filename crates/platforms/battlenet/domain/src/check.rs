//! Checking Battle.net CDN files without trusting where they came from (such
//! as lancache's cache): every file's name is a hash of its content.
//!
//! - `config/<h>`: `h` is the MD5 of the file.
//! - `<h>.index` (archive and patch indexes): `h` is the MD5 of the 28-byte
//!   footer.
//! - `data/<ekey>` (a loose file) and every file inside an archive: BLTE,
//!   checked by [`crate::blte::verify`] against its encoding key.
//!
//! [`Check`] does it over the bytes as they arrive, so a file of any size is
//! checked without holding it.

use crate::blte;
use alloc::vec::Vec;
use lanstash_domain::checksum::md5::{Md5, md5};

/// Bytes of an index footer whose hashes are 8 bytes (all indexes seen).
pub const INDEX_FOOTER: usize = 28;

/// A 32-character hex hash as bytes.
#[must_use]
pub fn hex16(hex: &str) -> Option<[u8; 16]> {
    let b = hex.as_bytes();
    if b.len() != 32 {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).and_then(|d| u8::try_from(d).ok());
    let mut out = [0u8; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = digit(b[2 * i])? << 4 | digit(b[2 * i + 1])?;
    }
    Some(out)
}

/// Whether `data` is the whole file at CDN path `target`
/// (`/<path>/{config,data,patch}/ab/cd/<hash>[.index]`). Paths of any other
/// shape are never trusted.
#[must_use]
pub fn whole_file(target: &str, data: &[u8]) -> bool {
    let mut c = Check::whole(target);
    c.feed(data);
    c.finish()
}

/// Whether bytes `start..start + data.len()` of an archive hold every file
/// listed (its bytes in the archive, its encoding key in hex) intact. Files
/// not wholly inside the range are not checked; bytes between files cannot be.
#[must_use]
pub fn archive_range(start: u64, data: &[u8], files: &[(u64, u64, &str)]) -> bool {
    let end = start + data.len() as u64;
    let inside: Vec<(u64, u64, &str)> = files.iter().copied().filter(|(a, b, _)| start <= *a && *b <= end).collect();
    let mut c = Check::range(start, &inside);
    c.feed(data);
    c.finish()
}

/// A check over a file's bytes as they arrive, in order, keeping at most an
/// index footer or a BLTE header: as [`whole_file`] and [`archive_range`].
#[derive(Debug, Clone)]
pub enum Check {
    /// A config: the MD5 of all of it.
    Config(Md5, [u8; 16]),
    /// An index: the MD5 of its last [`INDEX_FOOTER`] bytes.
    Index(Vec<u8>, [u8; 16]),
    /// A loose file, BLTE.
    Blte(blte::Verifier),
    /// Files inside an archive range: where the bytes are, and each file (its
    /// bytes, its check; `None` when its key is not a hash).
    Range(u64, Vec<(u64, u64, Option<blte::Verifier>)>),
    /// A path no rule covers: never passes.
    Never,
}

impl Check {
    /// The check of the whole file at CDN path `target`.
    #[must_use]
    pub fn whole(target: &str) -> Self {
        let mut parts = target.rsplit('/');
        let (Some(name), Some(_), Some(_), Some(kind)) = (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Self::Never;
        };
        let h = |s: &str| hex16(s);
        match (kind, name.strip_suffix(".index")) {
            ("data" | "patch", Some(hash)) => h(hash).map_or(Self::Never, |h| Self::Index(Vec::new(), h)),
            ("config", None) => h(name).map_or(Self::Never, |h| Self::Config(Md5::new(), h)),
            ("data", None) => h(name).map_or(Self::Never, |h| Self::Blte(blte::Verifier::new(h))),
            _ => Self::Never,
        }
    }

    /// The check of an archive range starting at `start`, holding `files`
    /// (each wholly inside it: bytes in the archive, encoding key in hex).
    #[must_use]
    pub fn range(start: u64, files: &[(u64, u64, &str)]) -> Self {
        Self::Range(start, files.iter().map(|(a, b, k)| (*a, *b, hex16(k).map(blte::Verifier::new))).collect())
    }

    /// The next bytes.
    pub fn feed(&mut self, data: &[u8]) {
        match self {
            Self::Config(m, _) => m.update(data),
            Self::Index(tail, _) => {
                tail.extend_from_slice(data);
                let excess = tail.len().saturating_sub(INDEX_FOOTER);
                tail.drain(..excess);
            }
            Self::Blte(v) => v.feed(data),
            Self::Range(at, files) => {
                let (from, to) = (*at, *at + data.len() as u64);
                for (a, b, v) in files.iter_mut() {
                    let (lo, hi) = ((*a).max(from), (*b).min(to));
                    if let (Some(v), true) = (v.as_mut(), lo < hi) {
                        let (lo, hi) =
                            (usize::try_from(lo - from).unwrap_or(0), usize::try_from(hi - from).unwrap_or(0));
                        v.feed(&data[lo..hi]);
                    }
                }
                *at = to;
            }
            Self::Never => {}
        }
    }

    /// Whether it can pass at all: a path a rule covers, a range whose every file has a hash key.
    /// One that cannot is no judgement on the bytes (a CDN download of it goes unchecked).
    #[must_use]
    pub fn checks(&self) -> bool {
        match self {
            Self::Never => false,
            Self::Range(_, files) => files.iter().all(|(_, _, v)| v.is_some()),
            Self::Config(..) | Self::Index(..) | Self::Blte(_) => true,
        }
    }

    /// Whether everything checked out.
    #[must_use]
    pub fn finish(&self) -> bool {
        match self {
            Self::Config(m, h) => m.finish() == *h,
            Self::Index(tail, h) => tail.len() == INDEX_FOOTER && md5(tail) == *h,
            Self::Blte(v) => v.finish(),
            Self::Range(at, files) => {
                files.iter().all(|(_, b, v)| *b <= *at && v.as_ref().is_some_and(blte::Verifier::finish))
            }
            Self::Never => false,
        }
    }
}
