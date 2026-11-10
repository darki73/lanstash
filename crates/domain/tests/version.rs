//! Tests for `lanstash_domain::version`.

use lanstash_domain::version::{newer, parse};

#[test]
fn versions_and_tags_are_read_alike() {
    assert_eq!(parse("0.2.0"), Some((0, 2, 0)));
    assert_eq!(parse("v1.10.3"), Some((1, 10, 3)));
    assert_eq!(parse(" v0.2.0-dev+abc "), Some((0, 2, 0)), "a suffix is not compared");
    for not in ["", "v", "1.2", "1.2.3.4", "a.b.c", "1..3"] {
        assert_eq!(parse(not), None, "{not:?}");
    }
}

#[test]
fn only_a_higher_release_is_newer() {
    assert!(newer("v0.2.0", "0.1.0"));
    assert!(newer("v0.10.0", "0.9.9"), "numbers, not text");
    assert!(!newer("v0.1.0", "0.1.0"), "the same");
    assert!(!newer("v0.1.0", "0.2.0-dev"), "a build ahead of the last release");
    assert!(!newer("nightly", "0.1.0"), "not a version");
}
