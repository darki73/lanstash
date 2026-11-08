//! A plan's pieces as lancache's nginx slice keys, and what a set of present
//! slices covers of them.
//!
//! lancache keys every slice as `<service><uri>bytes=<a>-<b>` (its
//! `CACHE_KEY=$cacheidentifier$uri$slice_range`, 1 MiB slices) and names the
//! file `md5(key)`. The plan lists every file an install downloads (whole
//! files, and byte ranges of archives), so each slice it needs has a known
//! key, and so a known file name.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::nginx_cache::{slice_key, slice_starts};

use crate::plan::Plan;

/// The slices a plan needs: each piece (bytes, slice numbers), and the keys.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wanted {
    /// Distinct slice keys, in the order first needed.
    pub keys: Vec<String>,
    /// Whole files: (bytes, indexes into `keys`).
    pub whole: Vec<(u64, Vec<usize>)>,
    /// Files inside archives: (bytes, indexes into `keys`).
    pub archived: Vec<(u64, Vec<usize>)>,
}

/// The nginx slice keys of everything a plan downloads, under the cache
/// identifier `service`, in slices of `slice` bytes. A whole file of unknown
/// size (0) counts as its first slice.
#[must_use]
pub fn wanted(plan: &Plan, service: &str, slice: u64) -> Wanted {
    let mut w = Wanted::default();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut slices = |target: &str, start: u64, end: u64| -> Vec<usize> {
        let base = format!("{service}{target}");
        slice_starts(start, end.max(start + 1), slice)
            .map(|s| {
                let key = slice_key(&base, s, slice);
                *index.entry(key).or_insert_with_key(|k| {
                    w.keys.push(k.clone());
                    w.keys.len() - 1
                })
            })
            .collect()
    };
    let whole: Vec<_> = plan.files.iter().map(|(t, size)| (*size, slices(t, 0, *size))).collect();
    let archived: Vec<_> = plan
        .parts
        .iter()
        .filter_map(|(a, span, _)| plan.archives.get(*a).map(|t| (span.len(), slices(t, span.start, span.end))))
        .collect();
    w.whole = whole;
    w.archived = archived;
    w
}

/// Wanted pieces of one kind (whole files, or files inside archives).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub wanted: u64,
    /// Those with every slice present.
    pub found: u64,
    pub bytes: u64,
    pub bytes_found: u64,
}

/// What `present` (by index into [`Wanted::keys`]) covers of `pieces`.
#[must_use]
pub fn tally(pieces: &[(u64, Vec<usize>)], present: &[bool]) -> Tally {
    pieces.iter().fold(Tally::default(), |mut t, (bytes, keys)| {
        t.wanted += 1;
        t.bytes += bytes;
        if keys.iter().all(|&k| present[k]) {
            t.found += 1;
            t.bytes_found += bytes;
        }
        t
    })
}
