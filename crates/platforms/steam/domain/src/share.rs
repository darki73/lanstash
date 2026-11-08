//! How much of an install lancache's old cache holds, estimated from a
//! sample of its chunks: an evenly spread sample ([`sample`]), each looked
//! up by the cache file of its first slice ([`first_slice`]: lancache keys
//! Steam chunks `steam/depot/<d>/chunk/<sha>` plus `bytes=0-1048575`),
//! the bytes found scaled up to the whole
//! ([`estimate`]). An estimate, so the page says "about".

use alloc::string::String;
use alloc::vec::Vec;

use lanstash_domain::nginx_cache::{cache_file_path, slice_key};

use crate::manifest::{Chunk, chunk_path};

/// lancache's slice size.
pub const SLICE: u64 = 1 << 20;

/// At most `max` of `items`, evenly spread (all of them when there are no more).
#[must_use]
pub fn sample<T>(items: &[T], max: usize) -> Vec<&T> {
    if max == 0 {
        return Vec::new();
    }
    if items.len() <= max {
        return items.iter().collect();
    }
    (0..max).map(|i| &items[i * items.len() / max]).collect()
}

/// The cache file (relative to the cache directory) of a chunk's first slice.
#[must_use]
pub fn first_slice(depot: u32, chunk: &Chunk) -> String {
    let base = alloc::format!("steam{}", chunk_path(depot, chunk));
    cache_file_path(&slice_key(&base, 0, SLICE))
}

/// `want` scaled by the share of sampled bytes found (`found` of `sampled`).
#[must_use]
pub fn estimate(want: u64, sampled: u64, found: u64) -> u64 {
    if sampled == 0 {
        return 0;
    }
    (u128::from(want) * u128::from(found.min(sampled)) / u128::from(sampled)) as u64
}
