//! Tests for `lanstash_dns_adguard::check`, with `check_host` answers as AdGuard
//! Home v0.107.62 gives them.

use lanstash_dns_adguard::check::{answered, unspecified};
use lanstash_domain::json::parse;
use lanstash_ports::dns::Answered;

fn of(json: &str) -> Answered {
    answered(&parse(json).unwrap())
}

fn to(a: &Answered) -> Option<&str> {
    a.to.as_deref()
}

#[test]
fn a_rewrite_answers_its_address_and_no_rule() {
    let direct = of(
        r#"{"reason":"Rewrite","rule":"","rules":[],"service_name":"","cname":"","ip_addrs":["10.0.0.210"],"filter_id":0}"#,
    );
    assert_eq!((to(&direct), direct.rule.as_deref()), (Some("10.0.0.210"), None));
    let through = of(
        r#"{"reason":"Rewrite","rule":"","rules":[],"service_name":"","cname":"lancache.local","ip_addrs":["10.0.0.210"],"filter_id":0}"#,
    );
    assert_eq!(to(&through), Some("10.0.0.210"), "the address it leads to, not the name between");
}

#[test]
fn a_rewrite_rule_answers_from_its_cname_or_its_text() {
    let short = of(
        r#"{"reason":"RewriteRule","rule":"||riot.example^$dnsrewrite=10.0.0.210","rules":[{"text":"||riot.example^$dnsrewrite=10.0.0.210","filter_list_id":0}],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!(to(&short), Some("10.0.0.210"));
    assert_eq!(short.rule.as_deref(), Some("||riot.example^$dnsrewrite=10.0.0.210"));
    let long = of(
        r#"{"reason":"RewriteRule","rule":"||lan.example^$dnsrewrite=NOERROR;A;10.0.0.210","rules":[{"text":"||lan.example^$dnsrewrite=NOERROR;A;10.0.0.210","filter_list_id":0}],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!(to(&long), Some("10.0.0.210"));
    let named = of(
        r#"{"reason":"RewriteRule","rule":"||cname.example^$dnsrewrite=lancache.local","rules":[{"text":"||cname.example^$dnsrewrite=lancache.local","filter_list_id":0}],"service_name":"","cname":"lancache.local","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!(to(&named), Some("lancache.local"));
    let more = of(r#"{"reason":"RewriteRule","rule":"||x.example^$dnsrewrite=10.0.0.9,important","ip_addrs":null}"#);
    assert_eq!(to(&more), Some("10.0.0.9"), "other modifiers after it");
}

#[test]
fn a_hosts_rule_answers_its_address_and_a_blocking_rule_nothing() {
    let hosts = of(
        r#"{"reason":"FilteredBlackList","rule":"10.0.0.210 hosts.example","rules":[{"text":"10.0.0.210 hosts.example","filter_list_id":0}],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!((to(&hosts), hosts.rule.as_deref()), (Some("10.0.0.210"), Some("10.0.0.210 hosts.example")));
    let blocked = of(
        r#"{"reason":"FilteredBlackList","rule":"||blocked.example^","rules":[{"text":"||blocked.example^","filter_list_id":0}],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!((to(&blocked), blocked.rule.as_deref()), (None, Some("||blocked.example^")));
    assert_eq!(
        to(&of(r#"{"reason":"FilteredBlackList","rule":"0.0.0.0 ads.example"}"#)),
        None,
        "blocked as a hosts rule"
    );
    assert!(unspecified("0.0.0.0") && unspecified("::") && !unspecified("10.0.0.210"));
}

#[test]
fn nothing_of_its_own_answers_nothing() {
    let none = of(
        r#"{"reason":"NotFilteredNotFound","rule":"","rules":[],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#,
    );
    assert_eq!(none, Answered { to: None, rule: None });
}
