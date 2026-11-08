//! Tests for `lanstash_steam_domain::share`.

use lanstash_domain::nginx_cache::cache_file_path;
use lanstash_steam_domain::manifest::Chunk;
use lanstash_steam_domain::share::{estimate, first_slice, sample};

#[test]
fn a_sample_is_spread_evenly_and_never_larger_than_asked() {
    let items: Vec<u32> = (0..10).collect();
    assert_eq!(sample(&items, 20).len(), 10);
    assert_eq!(sample(&items, 5).into_iter().copied().collect::<Vec<_>>(), [0, 2, 4, 6, 8]);
    assert_eq!(sample(&items, 3).into_iter().copied().collect::<Vec<_>>(), [0, 3, 6]);
    assert!(sample(&items, 0).is_empty());
    assert!(sample::<u32>(&[], 4).is_empty());
}

#[test]
fn the_first_slice_is_keyed_as_lancache_keys_it() {
    let c = Chunk { sha: [0x35; 20], crc: 0, offset: 0, original: 0, compressed: 1 };
    let key = format!("steam/depot/526871/chunk/{}bytes=0-1048575", "35".repeat(20));
    assert_eq!(first_slice(526_871, &c), cache_file_path(&key));
}

#[test]
fn the_estimate_scales_the_sample() {
    assert_eq!(estimate(1000, 200, 50), 250);
    assert_eq!(estimate(1000, 200, 200), 1000);
    assert_eq!(estimate(1000, 200, 900), 1000, "never more than all");
    assert_eq!(estimate(1000, 0, 0), 0);
    assert_eq!(estimate(u64::MAX, 2, 1), u64::MAX / 2);
}
