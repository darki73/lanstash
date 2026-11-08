//! Tests for `lanstash_domain::evict`.

use lanstash_domain::evict::*;

#[test]
fn limits_say_how_much_to_free() {
    let l = Limits { max_size: Some(100), keep_free: Some(KeepFree::Percent(10)) };
    assert_eq!(l.over_max_size(130), 30);
    assert_eq!(l.over_max_size(90), 0);
    assert_eq!(l.short_of_free(Disk { free: 40, total: 1000 }), 60);
    assert_eq!(l.short_of_free(Disk { free: 400, total: 1000 }), 0);
    assert_eq!(KeepFree::Bytes(7).bytes(1000), 7);
    assert_eq!(KeepFree::Percent(50).bytes(u64::MAX), u64::MAX / 2);
    assert!(Limits::default().is_off());
    assert_eq!(Limits::default().over_max_size(u64::MAX), 0);
}

#[test]
fn choose_takes_the_least_recently_used_until_enough() {
    let c = |bytes, last_used_ms| Candidate { bytes, last_used_ms };
    let cands = [c(10, 500), c(10, 100), c(10, 300), c(10, 900), c(50, 200)];
    assert_eq!(choose(&cands, 15, 1000), vec![1, 4]);
    assert_eq!(choose(&cands, 0, 1000), Vec::<usize>::new());
    // Used at or after 300: protected, even when that leaves too little.
    assert_eq!(choose(&cands, 1000, 300), vec![1, 4]);
    // Same time: the larger one first (fewer deletions).
    assert_eq!(choose(&[c(5, 1), c(9, 1)], 1, 10), vec![1]);
}

#[test]
fn recency_round_trips_and_skips_junk() {
    let mut r = Recency::default();
    r.segments.insert(3, 1_793_875_000_000);
    r.files.insert(0, 42);
    assert_eq!(Recency::parse(&r.encode()), r);
    let junk = Recency::parse("s 1 2\nx 1 2\ns one 2\ns 1\ns 1 2 3\n o 4 5 \n");
    assert_eq!((junk.segments.len(), junk.files.get(&4)), (1, Some(&5)));
}

#[test]
fn percentages() {
    assert_eq!(parse_percent("10%"), Some(10));
    assert_eq!(parse_percent("5 %"), Some(5));
    assert_eq!(parse_percent("0%"), None);
    assert_eq!(parse_percent("100%"), None);
    assert_eq!(parse_percent("10"), None);
}
