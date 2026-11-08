//! Tests for `lanstash_app::dns::covered`: names the server sends to lanstash
//! without a rewrite of their own are not missing.

use super::*;
use lanstash_app::dns::covered::{BY_REWRITE, sample};

fn by(to: Option<&str>, rule: Option<&str>) -> Answered {
    Answered { to: to.map(str::to_owned), rule: rule.map(str::to_owned) }
}

#[test]
fn a_pattern_a_rule_sends_to_lanstash_is_covered_and_apply_leaves_it() {
    let rule = "||steamcontent.com^$dnsrewrite=cache.lan";
    let server = Arc::new(Server {
        answers: Some(vec![
            // Asked as a name under it: a rule to lanstash's host name, which a rewrite leads to lanstash.
            ("lanstash.steamcontent.com".into(), by(Some("cache.lan"), Some(rule))),
            // A wider rewrite.
            ("lancache.steamcontent.com".into(), by(Some("10.0.0.10"), None)),
            // A rule elsewhere, and a blocked name: still missing.
            ("lanstash.blizzard.com".into(), by(Some("10.0.0.99"), Some("10.0.0.99 x"))),
            ("level3.blizzard.com".into(), by(None, Some("||level3.blizzard.com^"))),
        ]),
        ..Server::default()
    });
    *server.rewrites.lock().unwrap() = vec![rw("cache.lan", "10.0.0.10")];
    let l = link(Arc::clone(&server), Some("10.0.0.10"));
    let plan = l.report().unwrap().plan.unwrap();
    assert_eq!(plan.missing, ["*.blizzard.com", "level3.blizzard.com"]);
    assert_eq!(
        plan.covered,
        [
            ("*.steamcontent.com".to_owned(), rule.to_owned()),
            ("lancache.steamcontent.com".to_owned(), BY_REWRITE.to_owned())
        ]
    );
    server.calls.lock().unwrap().clear();
    assert_eq!(l.apply().unwrap().added, 2);
    let added: Vec<String> = server.calls.lock().unwrap().iter().filter(|c| c.starts_with("add")).cloned().collect();
    assert_eq!(added, ["add *.blizzard.com", "add level3.blizzard.com"], "nothing for the covered ones");
}

#[test]
fn a_server_that_cannot_say_or_fails_leaves_them_missing() {
    let quiet = Arc::new(Server::default());
    assert_eq!(link(quiet, Some("10.0.0.10")).report().unwrap().plan.unwrap().missing.len(), 4);
    let failing = Arc::new(Server { answers: Some(vec![]), ..Server::default() });
    let plan = link(Arc::clone(&failing), Some("10.0.0.10")).report().unwrap().plan.unwrap();
    assert_eq!((plan.missing.len(), plan.covered.len()), (4, 0));
    assert_eq!(failing.calls.lock().unwrap().len(), 4, "asked once a missing pattern");
}

#[test]
fn a_pattern_is_asked_about_as_a_name() {
    assert_eq!(sample("lancache.steamcontent.com"), "lancache.steamcontent.com");
    assert_eq!(sample("*.blizzard.com"), "lanstash.blizzard.com");
}
