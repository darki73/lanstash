//! Tests for `lanstash_app::serve::refused`: the hosts the cache refused.

use super::*;
use lanstash_app::serve::refused::{MAX_HOSTS, RefusedHosts};

#[test]
fn a_refused_host_is_counted_by_name() {
    let r = rig();
    for _ in 0..2 {
        assert_eq!(r.get("steampipe.akamaized.net", "/depot/1/chunk/00", None).status, 403);
    }
    r.get("other.example", "/", None);
    r.get(STEAM, STEAM_PATH, None);
    let (hosts, others) = r.cache.refused.list();
    assert_eq!(hosts, [("steampipe.akamaized.net".to_owned(), 2), ("other.example".to_owned(), 1)]);
    assert_eq!(others, 0);
}

#[test]
fn keeps_a_bounded_number_of_names() {
    let t = RefusedHosts::default();
    for i in 0..=MAX_HOSTS {
        t.note(&format!("h{i}.example"));
    }
    t.note("h0.example");
    let (hosts, others) = t.list();
    assert_eq!((hosts.len(), others), (MAX_HOSTS, 1), "the last new one only counted");
    assert_eq!(hosts[0], ("h0.example".to_owned(), 2), "most refused first");
}
