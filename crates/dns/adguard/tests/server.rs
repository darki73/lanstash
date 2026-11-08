//! Tests for `lanstash_dns_adguard::server`, against answers shaped as AdGuard
//! Home v0.107.62 gives them.

use std::sync::{Arc, Mutex};

use lanstash_dns_adguard::server::AdGuard;
use lanstash_ports::dns::{DnsServer, Rewrite};
use lanstash_ports::web::{Endpoint, Tls, Web, WebError, WebRequest, WebResponse, untrusted_in};

/// How a request asked for TLS: none (plain HTTP), or its pin and whether unchecked.
type Asked = Option<(Option<String>, bool)>;

/// Answers each request with the next canned response; keeps what was asked.
#[derive(Default)]
struct Canned {
    answers: Mutex<Vec<(u16, &'static str)>>,
    asked: Mutex<Vec<String>>,
    tls: Mutex<Vec<Asked>>,
}

impl Web for Canned {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let auth = req.headers.iter().find(|(n, _)| *n == "Authorization").map_or("", |(_, v)| *v);
        self.asked.lock().unwrap().push(format!(
            "{} {}:{}{} {auth} {}",
            req.method,
            req.host,
            req.port,
            req.target,
            String::from_utf8_lossy(req.body)
        ));
        self.tls.lock().unwrap().push(req.tls.map(|Tls { pin, unchecked }| (pin.map(str::to_owned), unchecked)));
        let (status, body) = self.answers.lock().unwrap().remove(0);
        if status == 0 {
            return Err(WebError {
                message: "10.0.0.53: its certificate is not trusted".into(),
                untrusted: Some(body.into()),
            });
        }
        Ok(WebResponse { status, body: body.as_bytes().to_vec() })
    }
}

fn plain() -> Endpoint {
    Endpoint { host: "10.0.0.53".into(), port: 3000, https: false, pin: None, unchecked: false }
}

fn adguard_at(at: Endpoint, answers: Vec<(u16, &'static str)>) -> (AdGuard, Arc<Canned>) {
    let web = Arc::new(Canned { answers: Mutex::new(answers), ..Canned::default() });
    (AdGuard::new(web.clone(), at, "lanstash", "secret"), web)
}

fn adguard(answers: Vec<(u16, &'static str)>) -> (AdGuard, Arc<Canned>) {
    adguard_at(plain(), answers)
}

#[test]
fn speaks_https_trusting_a_pin_or_nothing_and_names_an_untrusted_certificate() {
    let https = Endpoint { port: 443, https: true, ..plain() };
    let (a, web) = adguard_at(https.clone(), vec![(0, "AB:CD:EF"), (200, "[]")]);
    let e = a.rewrites().unwrap_err();
    assert_eq!(untrusted_in(&e), Some("AB:CD:EF"), "{e}");
    assert!(a.rewrites().unwrap().is_empty());
    assert_eq!(web.tls.lock().unwrap()[0], Some((None, false)), "the public roots");
    let (a, web) = adguard_at(Endpoint { pin: Some("AB:CD:EF".into()), ..https.clone() }, vec![(200, "[]")]);
    a.rewrites().unwrap();
    assert_eq!(web.tls.lock().unwrap()[0], Some((Some("AB:CD:EF".into()), false)), "pinned");
    let (a, web) = adguard_at(Endpoint { unchecked: true, ..https }, vec![(200, "[]")]);
    a.rewrites().unwrap();
    assert_eq!(web.tls.lock().unwrap()[0], Some((None, true)), "not checked");
    let (a, web) = adguard(vec![(200, "[]")]);
    a.rewrites().unwrap();
    assert_eq!(web.tls.lock().unwrap()[0], None, "plain HTTP");
    // Plain HTTP to an HTTPS-only admin page, as AdGuard Home answers it either way.
    let (a, _) = adguard(vec![
        (400, "Client sent an HTTP request to an HTTPS server.\n"),
        (307, "<a href=\"https://127.0.0.1/control/rewrite/list\">Temporary Redirect</a>.\n"),
    ]);
    for _ in 0..2 {
        assert!(a.rewrites().unwrap_err().ends_with("is HTTPS-only: choose HTTPS (and its HTTPS port)"));
    }
}

#[test]
fn lists_adds_and_removes_rewrites_signed_in() {
    let list = r#"[{"domain":"lancache.steamcontent.com","answer":"10.0.0.210"},{"domain":"*.steamcontent.com","answer":"10.0.0.210"},{"domain":"printer.home","answer":"10.0.0.5"}]"#;
    let (a, web) = adguard(vec![(200, list), (200, "OK\n"), (200, "OK\n")]);
    let got = a.rewrites().unwrap();
    assert_eq!(got.len(), 3);
    assert_eq!(got[1], Rewrite { pattern: "*.steamcontent.com".into(), answer: "10.0.0.210".into() });
    let r = Rewrite { pattern: "*.blizzard.com".into(), answer: "10.0.0.210".into() };
    a.add(&r).unwrap();
    a.remove(&r).unwrap();
    let asked = web.asked.lock().unwrap();
    // "lanstash:secret" in base64.
    let auth = "Basic bGFuc3Rhc2g6c2VjcmV0";
    assert_eq!(asked[0], format!("GET 10.0.0.53:3000/control/rewrite/list {auth} "));
    assert_eq!(
        asked[1],
        format!(
            r#"POST 10.0.0.53:3000/control/rewrite/add {auth} {{"domain":"*.blizzard.com","answer":"10.0.0.210"}}"#
        )
    );
    assert!(asked[2].starts_with("POST 10.0.0.53:3000/control/rewrite/delete "));
    assert_eq!(a.kind(), "AdGuard Home");
    assert!(!format!("{a:?}").contains("secret") && !format!("{a:?}").contains("bGFu"), "no credentials in Debug");
}

#[test]
fn reads_the_query_log_as_lookups() {
    let log = r#"{"data":[
        {"question":{"class":"IN","name":"lancache.steamcontent.com","type":"A"},"client":"10.0.1.100","reason":"Rewrite","rules":[],"answer":[{"type":"A","value":"10.0.0.210","ttl":10}],"status":"NOERROR","time":"2026-11-06T15:38:42.4389107Z"},
        {"question":{"class":"IN","name":"steampipe.akamaized.net","type":"A"},"client":"10.0.1.100","reason":"NotFilteredNotFound","rules":[],"answer":[{"type":"CNAME","value":"edge1.cdn.example.","ttl":264},{"type":"A","value":"203.0.113.94","ttl":2}],"status":"NOERROR","time":"2026-11-06T15:38:42.4098617Z"},
        {"question":{"class":"IN","type":"A"},"client":"x"},
        {"question":{"class":"IN","name":"hosts.example","type":"A"},"client":"10.0.1.100","reason":"FilteredBlackList","answer":[{"type":"A","value":"10.0.0.210"}],"time":"2026-11-06T15:38:41Z"},
        {"question":{"class":"IN","name":"hosts.example","type":"AAAA"},"client":"10.0.1.100","reason":"FilteredBlackList","answer":[{"type":"AAAA","value":"::"}],"time":"2026-11-06T15:38:41Z"},
        {"question":{"class":"IN","name":"lan.example","type":"A"},"client":"10.0.1.100","reason":"RewriteRule","answer":[{"type":"A","value":"10.0.0.210"}],"time":"2026-11-06T15:38:41Z"}
    ],"oldest":"2026-11-06T15:38:42.4098617Z"}"#;
    let (a, web) = adguard(vec![(200, log)]);
    let got = a.lookups(500).unwrap();
    assert_eq!(got.len(), 5, "an entry without a name is left out");
    let by_rules: Vec<bool> = got[2..].iter().map(|l| l.rewritten).collect();
    assert_eq!(
        by_rules,
        [true, false, true],
        "a hosts rule's address and a rewrite rule answer; a blocked one does not"
    );
    assert_eq!(got[0].name, "lancache.steamcontent.com");
    assert!(got[0].rewritten && !got[1].rewritten);
    assert_eq!(got[1].answers, ["203.0.113.94"], "addresses only, not the CNAME");
    assert_eq!((got[1].client.as_str(), got[0].at_ms), ("10.0.1.100", 1_793_979_522_438));
    assert!(web.asked.lock().unwrap()[0].starts_with("GET 10.0.0.53:3000/control/querylog?limit=500 "));
}

#[test]
fn asks_how_a_name_is_answered_only_of_host_names() {
    let answer = r#"{"reason":"RewriteRule","rule":"||riot.example^$dnsrewrite=10.0.0.210","rules":[{"text":"||riot.example^$dnsrewrite=10.0.0.210","filter_list_id":0}],"service_name":"","cname":"","ip_addrs":null,"filter_id":0}"#;
    let (a, web) = adguard(vec![(200, answer)]);
    let got = a.check("riot.example").unwrap().unwrap();
    assert_eq!(got.to.as_deref(), Some("10.0.0.210"));
    assert!(
        web.asked.lock().unwrap()[0].starts_with("GET 10.0.0.53:3000/control/filtering/check_host?name=riot.example ")
    );
    assert!(a.check("a b&c=d").unwrap().is_err(), "not a host name: not asked");
    assert_eq!(web.asked.lock().unwrap().len(), 1);
}

#[test]
fn says_why_a_request_failed() {
    let (a, _) = adguard(vec![(403, "Forbidden"), (500, "boom\n"), (200, "not json")]);
    assert_eq!(a.rewrites().unwrap_err(), "AdGuard Home refused lanstash's user name and password");
    let r = Rewrite { pattern: "x".into(), answer: "1.2.3.4".into() };
    assert_eq!(a.add(&r).unwrap_err(), "AdGuard Home answered 500 to POST /control/rewrite/add: boom");
    assert!(a.rewrites().unwrap_err().starts_with("AdGuard Home's /control/rewrite/list: "));
}
