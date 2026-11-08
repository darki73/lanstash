//! Tests for `lanstash_domain::service::hosts`.

use lanstash_domain::service::hosts::*;

#[test]
fn classifies_hosts_like_the_cache_domains_lists() {
    let mut r = HostRules::default();
    let n = r.add_list("blizzard", "# comment\n*.cdn.blizzard.com\nlevel3.blizzard.com\n\n");
    assert_eq!(n, 2);
    r.add_list("epicgames", "download.epicgames.com  # inline comment\n");
    r.add_host("steam", "steamcontent.com");
    assert_eq!(r.classify("us.cdn.blizzard.com"), Some("blizzard"));
    assert_eq!(r.classify("cdn.blizzard.com"), None, "*.x does not match x itself");
    assert_eq!(r.classify("LEVEL3.Blizzard.com."), Some("blizzard"));
    assert_eq!(r.classify("download.epicgames.com"), Some("epicgames"));
    assert_eq!(r.classify("cache1-fra1.steamcontent.com"), Some("steam"));
    assert_eq!(r.classify("evilsteamcontent.com"), None);
    assert_eq!(r.classify("example.com"), None);
    assert_eq!(r.services(), ["blizzard", "epicgames", "steam"]);
    assert_eq!((r.len(), r.is_empty()), (4, false));
}
