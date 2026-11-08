//! Tests for `lanstash_app::dns::setup`: a setup is checked, tried, then kept.

use super::*;
use lanstash_app::dns::setup::{check, pin};

fn kinds() -> Vec<DnsKind> {
    vec![
        DnsKind { id: "test".into(), name: "Test".into(), port: 3000, https_port: 443, user: true },
        DnsKind { id: "pw".into(), name: "Password only".into(), port: 80, https_port: 443, user: false },
    ]
}

#[test]
fn a_setup_is_tidied_or_refused_with_why() {
    let tidy = check(
        &DnsConfig {
            kind: " test ".into(),
            host: " Dns.Lan ".into(),
            port: 3000,
            https: false,
            pin: Some("ab".into()),
            unchecked: true,
            user: " lanstash ".into(),
            answer: Some(" ".into()),
            via: Some(" ".into()),
        },
        &kinds(),
    )
    .unwrap();
    assert_eq!(
        tidy,
        DnsConfig { host: "dns.lan".into(), ..config(None) },
        "tidied; a pin and no checking mean nothing over plain HTTP"
    );
    let fp = "ab cd ef 01 23 45 67 89 ab cd ef 01 23 45 67 89 ab cd ef 01 23 45 67 89 ab cd ef 01 23 45 67 89";
    let https = check(&DnsConfig { https: true, pin: Some(fp.into()), ..config(None) }, &kinds()).unwrap();
    assert_eq!(https.pin.as_deref(), Some(pin(fp).as_str()));
    assert!(https.pin.as_deref().unwrap().starts_with("AB:CD:EF:01"));
    assert!(check(&DnsConfig { https: true, pin: Some("AB:CD".into()), ..config(None) }, &kinds()).is_err());
    let open = check(&DnsConfig { https: true, unchecked: true, ..config(None) }, &kinds()).unwrap();
    assert!(open.https && open.unchecked);
    assert_eq!(
        check(
            &DnsConfig { kind: " test ".into(), host: " Dns.Lan ".into(), user: " lanstash ".into(), ..config(None) },
            &kinds()
        )
        .unwrap()
        .host,
        "dns.lan"
    );
    let bad = |c: DnsConfig| check(&c, &kinds()).unwrap_err();
    assert!(bad(DnsConfig { kind: "pihole".into(), ..config(None) }).contains("unknown DNS server"));
    assert!(bad(DnsConfig { host: "http://10.0.0.1/".into(), ..config(None) }).contains("without http://"));
    assert!(bad(DnsConfig { port: 0, ..config(None) }).contains("port"));
    assert!(bad(DnsConfig { user: " ".into(), ..config(None) }).contains("user"));
    let password_only = check(&DnsConfig { kind: "pw".into(), user: "someone".into(), ..config(None) }, &kinds());
    assert_eq!(password_only.map(|c| c.user), Ok(String::new()), "a kind without users keeps none");
    assert!(check(&DnsConfig { kind: "pw".into(), user: String::new(), ..config(None) }, &kinds()).is_ok());
    assert!(bad(config(Some("10.0.0.300"))).contains("neither an IP address nor a host name"));
    assert!(check(&config(Some("lancache.local.example")), &kinds()).is_ok(), "a name that leads to lanstash");
    assert!(check(&config(Some("fd00::2")), &kinds()).is_ok());
}

#[test]
fn only_a_setup_that_works_is_kept_and_a_test_keeps_nothing() {
    let server = Arc::new(Server::default());
    *server.rewrites.lock().unwrap() = vec![rw("printer.lan", "10.0.0.5")];
    let l = link_with(Arc::clone(&server), None);
    assert_eq!(l.configure(&config(None), None, true).unwrap_err(), "a password is needed");
    assert_eq!(l.configure(&config(None), Some("wrong"), true).unwrap_err(), "refused the password");
    assert!(l.setup().config().is_none(), "a setup that fails is not kept");
    let tried = l.configure(&config(None), Some("pw"), false).unwrap();
    assert_eq!((tried.kind.as_str(), tried.rewrites), ("test server", 1));
    assert!(l.setup().config().is_none(), "a test keeps nothing");
    l.configure(&config(Some("10.0.0.10")), Some("pw"), true).unwrap();
    assert_eq!(l.setup().config(), Some(config(Some("10.0.0.10"))));
    // A new address without the password: the kept one is used.
    l.configure(&config(None), None, true).unwrap();
    assert_eq!((l.setup().config(), l.setup().password().as_deref()), (Some(config(None)), Some("pw")));
    l.forget().unwrap();
    assert!(l.report().is_err());
}

#[test]
fn the_kept_password_goes_only_to_the_server_it_was_kept_for() {
    let server = Arc::new(Server::default());
    let l = link_with(Arc::clone(&server), None);
    l.configure(&config(None), Some("pw"), true).unwrap();
    let others = [
        DnsConfig { host: "elsewhere.lan".into(), ..config(None) },
        DnsConfig { port: 80, ..config(None) },
        DnsConfig { user: "other".into(), ..config(None) },
        DnsConfig { https: true, unchecked: true, ..config(None) },
    ];
    for other in &others {
        assert_eq!(l.configure(other, None, false).unwrap_err(), "a password is needed for another server");
    }
    assert!(l.configure(&config(Some("10.0.0.10")), None, false).is_ok(), "the same server, another answer");
}

#[test]
fn the_server_is_never_this_machines_loopback() {
    for host in ["localhost", "127.0.0.1", "127.8.9.10", "[::1]", "::ffff:127.0.0.1", "0.0.0.0", "a.localhost"] {
        let e = check(&DnsConfig { host: host.into(), ..config(None) }, &kinds()).unwrap_err();
        assert!(e.contains("loopback"), "{host}: {e}");
    }
}

#[test]
fn a_user_name_holds_no_line_break_or_colon() {
    for user in ["a\nunchecked = true", "a:b", "a\tb"] {
        let e = check(&DnsConfig { user: user.into(), ..config(None) }, &kinds()).unwrap_err();
        assert!(e.contains("user name"), "{user:?}: {e}");
    }
}
