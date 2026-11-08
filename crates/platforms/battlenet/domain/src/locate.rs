//! Where a build's files lie on the CDN, and which requests fetch them.
//!
//! A file (an encoded key) is either inside an archive, at the offset and
//! size its archive index gives ([`crate::tact::parse_index`]), or a loose
//! file under `data`, listed in the file index. A client fetches archive
//! files with ranged requests, so a prefill does too: [`ranges`] sorts each
//! archive's files and merges those less than `gap` bytes apart, trading a
//! little over-fetch for far fewer requests.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::tact::IndexEntry;

/// Where one file lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// In archive number `archive` (of the CDN config's list).
    Archive { archive: usize, offset: u64, size: u64 },
    /// A loose file of `size` bytes (0 when unknown).
    Loose { size: u64 },
    /// In no archive and not in the file index: the CDN does not have it.
    Absent,
}

impl Place {
    /// Its bytes (0 for an absent file or a loose one of unknown size).
    #[must_use]
    pub fn size(&self) -> u64 {
        match *self {
            Self::Archive { size, .. } | Self::Loose { size } => size,
            Self::Absent => 0,
        }
    }
}

/// A file and where it lies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub key: String,
    pub place: Place,
}

/// Places the `wanted` files (key, size as far as known; repeated keys
/// once): the archives first, then the file index. Without a file index
/// (`loose` is `None`) a file in no archive is taken to be loose.
#[must_use]
pub fn locate(wanted: &[(String, u64)], archives: &[Vec<IndexEntry>], loose: Option<&[IndexEntry]>) -> Vec<Located> {
    let mut at: BTreeMap<&str, usize> = BTreeMap::new();
    let mut out: Vec<Located> = Vec::with_capacity(wanted.len());
    for (key, size) in wanted {
        if !at.contains_key(key.as_str()) {
            at.insert(key, out.len());
            out.push(Located { key: key.clone(), place: Place::Loose { size: *size } });
        }
    }
    let mut placed = alloc::vec![false; out.len()];
    for (archive, entries) in archives.iter().enumerate() {
        for e in entries {
            if let (Some(&i), Some(offset)) = (at.get(e.key.as_str()), e.offset)
                && !placed[i]
            {
                out[i].place = Place::Archive { archive, offset, size: e.size };
                placed[i] = true;
            }
        }
    }
    if let Some(files) = loose {
        let mut listed = alloc::vec![false; out.len()];
        for e in files {
            if let Some(&i) = at.get(e.key.as_str())
                && !placed[i]
            {
                out[i].place = Place::Loose { size: e.size };
                listed[i] = true;
            }
        }
        for (i, l) in out.iter_mut().enumerate() {
            if !placed[i] && !listed[i] {
                l.place = Place::Absent;
            }
        }
    }
    out
}

/// One ranged request: bytes `start..end` of archive number `archive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub archive: usize,
    pub start: u64,
    pub end: u64,
}

/// The ranged requests that fetch every archive file of `files`: per
/// archive, by offset, files at most `gap` bytes apart in one request.
#[must_use]
pub fn ranges(files: &[Located], gap: u64) -> Vec<Range> {
    let mut parts: Vec<Range> = files
        .iter()
        .filter_map(|f| match f.place {
            Place::Archive { archive, offset, size } if size > 0 => {
                Some(Range { archive, start: offset, end: offset.saturating_add(size) })
            }
            _ => None,
        })
        .collect();
    parts.sort_unstable_by_key(|r| (r.archive, r.start));
    let mut out: Vec<Range> = Vec::new();
    for p in parts {
        match out.last_mut() {
            Some(last) if last.archive == p.archive && p.start <= last.end.saturating_add(gap) => {
                last.end = last.end.max(p.end);
            }
            _ => out.push(p),
        }
    }
    out
}

/// The bytes of `parts` (sorted by start) that lie wholly in `stored`
/// (sorted by start, may touch or overlap): one sweep over both.
#[must_use]
pub fn held_bytes(parts: &[lanstash_domain::extent::Span], stored: &[lanstash_domain::extent::Span]) -> u64 {
    // Merge `stored` into runs as the sweep goes.
    let mut runs = stored.iter().peekable();
    let (mut run_start, mut run_end) = (0u64, 0u64);
    let mut held = 0;
    for p in parts {
        while run_end < p.end {
            let Some(s) = runs.peek() else { break };
            if s.start > run_end {
                if s.start > p.start {
                    break; // the next run starts after this part does: it cannot hold it
                }
                (run_start, run_end) = (s.start, s.end);
            } else {
                run_end = run_end.max(s.end);
            }
            runs.next();
        }
        if run_start <= p.start && p.end <= run_end {
            held += p.end - p.start;
        }
    }
    held
}
