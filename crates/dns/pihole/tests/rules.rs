//! Tests for `lanstash_dns_pihole::rules`: rewrites as Pi-hole rules and records,
//! in the shapes Pi-hole 2026.10.0 takes and gives back.

use lanstash_dns_pihole::rules::{encode, from_cname, from_host, from_regex, is_address, regex};
use lanstash_ports::dns::Rewrite;

fn rw(pattern: &str, answer: &str) -> Rewrite {
    Rewrite { pattern: pattern.into(), answer: answer.into() }
}

#[test]
fn a_rewrite_to_an_address_is_a_reply_rule_and_reads_back() {
    assert_eq!(regex("lancache.riotcdn.net", "10.0.0.210").unwrap(), "^lancache[.]riotcdn[.]net$;reply=10.0.0.210");
    assert_eq!(regex("*.dyn.riotcdn.net", "10.0.0.210").unwrap(), "[.]dyn[.]riotcdn[.]net$;reply=10.0.0.210");
    for p in ["lancache.riotcdn.net", "*.dyn.riotcdn.net"] {
        assert_eq!(from_regex(&regex(p, "fd00::2").unwrap()), Some(rw(p, "fd00::2")), "{p}");
    }
    assert!(regex("a b", "10.0.0.1").is_err() && regex("x.example", "lanstash.lan").is_err());
    assert!(!regex("Steam.Example", "10.0.0.1").unwrap().contains('S'), "lower case");
}

#[test]
fn rules_a_person_wrote_read_back_when_they_are_rewrites_and_others_do_not() {
    assert_eq!(from_regex(r"^cdn\.example\.com$;reply=10.0.0.9"), Some(rw("cdn.example.com", "10.0.0.9")));
    assert_eq!(from_regex(r"\.example\.com$;reply=10.0.0.9"), Some(rw("*.example.com", "10.0.0.9")));
    assert_eq!(from_regex("^x[.]example$;querytype=A;reply=10.0.0.9"), Some(rw("x.example", "10.0.0.9")));
    for other in [
        "(^|[.])steamcontent[.]com$;reply=10.0.0.210", // both the name and its subdomains: not one pattern
        "^ads[.]example$",                             // blocks
        "^x[.]example$;reply=NXDOMAIN",
        "^x.*[.]example$;reply=10.0.0.9",
    ] {
        assert_eq!(from_regex(other), None, "{other}");
    }
}

#[test]
fn local_records_and_cnames_read_back() {
    assert_eq!(
        from_host("10.0.0.210 lancache.steamcontent.com lancache.local"),
        [rw("lancache.steamcontent.com", "10.0.0.210"), rw("lancache.local", "10.0.0.210")]
    );
    assert!(from_host("not-an-address name").is_empty());
    assert_eq!(from_cname("level3.blizzard.com,lancache.local"), Some(rw("level3.blizzard.com", "lancache.local")));
    assert_eq!(from_cname("a.example,b.example,300"), Some(rw("a.example", "b.example")), "with a TTL");
    assert_eq!(from_cname("broken"), None);
}

#[test]
fn entries_are_encoded_for_a_path() {
    assert_eq!(encode("10.0.0.210 lanstash.lan"), "10.0.0.210%20lanstash.lan");
    assert_eq!(encode("^a[.]b$;reply=1.2.3.4"), "%5Ea%5B.%5Db%24%3Breply%3D1.2.3.4");
    assert_eq!(encode("a.example,b.example"), "a.example%2Cb.example");
    assert!(is_address("10.0.0.210") && is_address("::") && !is_address("lanstash.lan") && !is_address("1.2.3.400"));
}
