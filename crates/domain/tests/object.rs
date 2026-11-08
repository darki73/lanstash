//! Tests for `lanstash_domain::object`.

use lanstash_domain::object::*;

#[test]
fn keys_ignore_query_and_host() {
    assert_eq!(path_of("/tpr/hs/data/ab/cd/abcd?x=1#f"), "/tpr/hs/data/ab/cd/abcd");
    assert_eq!(object_key("blizzard", path_of("/tpr/hs/a?token=9")), "blizzard/tpr/hs/a");
    // Same format as v1 Steam keys, so imported segments stay valid.
    assert_eq!(object_key("steam", "/depot/1/chunk/ab"), "steam/depot/1/chunk/ab");
}

#[test]
fn components_are_safe() {
    assert_eq!(safe_component("8efccb8531df.index"), "8efccb8531df.index");
    assert_eq!(safe_component("AB_CD-1.chunk"), "AB_CD-1.chunk");
    assert_eq!(safe_component("a b/c\\d:e"), "a%20b%2Fc%5Cd%3Ae");
    assert_eq!(safe_component("%41"), "%2541", "percent itself is encoded");
    for bad in ["", ".", ".."] {
        let c = safe_component(bad);
        assert!(c.starts_with("_h_") && c.len() == 35, "{bad:?} -> {c}");
    }
    let long = "x".repeat(MAX_COMPONENT + 1);
    assert!(safe_component(&long).starts_with("_h_"));
    assert_eq!(game_dir(""), "_");
    assert_eq!(game_dir("hs"), "hs");
    assert_eq!(game_dir("../etc"), "..%2Fetc");
}

#[test]
fn paths_cannot_escape() {
    assert_eq!(safe_rel_path("/tpr/hs/data/8e/fc/8efc"), "tpr/hs/data/8e/fc/8efc");
    assert_eq!(safe_rel_path("//a//b/"), "a/b");
    let p = safe_rel_path("/a/../../etc/passwd");
    assert!(!p.split('/').any(|c| c == ".." || c == "."), "{p}");
    assert!(safe_rel_path("/").starts_with("_h_"));
    let deep: String = (0..=MAX_DEPTH).map(|i| format!("/d{i}")).collect();
    assert!(safe_rel_path(&deep).starts_with("_h_"));
    for p in [safe_rel_path("/x/%2e%2e/y"), fallback_rel_path("blizzard/tpr/x")] {
        assert!(p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/%".contains(&b)), "{p}");
    }
}

#[test]
fn a_path_spelled_two_ways_is_not_plain() {
    for p in ["/tpr/x1/../wow/data", "/a/./b", "/a/%2E%2e/b", "/a%2Fb", "/a\\b", "/a/%5cb", "/.."] {
        assert!(!plain_path(p), "{p}");
    }
    for p in ["/tpr/wow/data/8e/fc/8efc", "/depot/1/chunk/ab", "/a/b.c/..d", "/a/...", "/"] {
        assert!(plain_path(p), "{p}");
    }
}
