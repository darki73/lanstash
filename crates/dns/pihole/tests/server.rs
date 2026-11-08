//! Tests for `lanstash_dns_pihole::server`, against a Pi-hole in memory that
//! answers its v6 API the way Pi-hole 2026.10.0 answers it.

use std::sync::{Arc, Mutex};

use lanstash_dns_pihole::server::PiHole;
use lanstash_ports::dns::{DnsServer, Rewrite};
use lanstash_ports::web::{Endpoint, Web, WebError, WebRequest, WebResponse};

#[derive(Default)]
struct State {
    regex: Vec<String>,
    hosts: Vec<String>,
    cnames: Vec<String>,
    queries: String,
    sessions: Vec<String>,
    asked: Vec<String>,
}

/// A Pi-hole whose password is `secret`.
#[derive(Default)]
struct Fake(Mutex<State>);

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn list(items: &[String]) -> String {
    items.iter().map(|i| format!("\"{}\"", i.replace('\\', "\\\\"))).collect::<Vec<_>>().join(",")
}

impl Web for Fake {
    fn send(&self, req: &WebRequest<'_>) -> Result<WebResponse, WebError> {
        let mut st = self.0.lock().unwrap();
        let sid = req.headers.iter().find(|(n, _)| *n == "X-FTL-SID").map(|(_, v)| (*v).to_owned());
        st.asked.push(format!("{} {}", req.method, req.target));
        let body = String::from_utf8_lossy(req.body).into_owned();
        let ok = |status: u16, body: &str| Ok(WebResponse { status, body: body.as_bytes().to_vec() });
        if req.target == "/api/auth" {
            return match req.method {
                "POST" if body.contains("\"secret\"") => {
                    let sid = format!("sid{}", st.asked.len());
                    st.sessions.push(sid.clone());
                    ok(200, &format!(r#"{{"session":{{"valid":true,"totp":false,"sid":"{sid}","validity":1800}}}}"#))
                }
                "POST" => {
                    ok(401, r#"{"session":{"valid":false,"sid":null,"validity":-1,"message":"password incorrect"}}"#)
                }
                _ => {
                    st.sessions.retain(|s| Some(s) != sid.as_ref());
                    ok(204, "")
                }
            };
        }
        if !sid.as_ref().is_some_and(|s| st.sessions.contains(s)) {
            return ok(401, r#"{"error":{"key":"unauthorized","message":"Unauthorized"}}"#);
        }
        let (path, entry) = req.target.rsplit_once('/').map_or((req.target, String::new()), |(p, e)| (p, decode(e)));
        match (req.method, req.target, path) {
            ("GET", "/api/domains/deny/regex", _) => {
                let items: Vec<String> = st
                    .regex
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        format!(r#"{{"domain":"{}","enabled":true,"id":{}}}"#, r.replace('\\', "\\\\"), i + 1)
                    })
                    .collect();
                ok(200, &format!(r#"{{"domains":[{}]}}"#, items.join(",")))
            }
            ("POST", "/api/domains/deny/regex", _) => {
                let rule = body.split("\"domain\":\"").nth(1).unwrap().split('"').next().unwrap().to_owned();
                st.regex.push(rule);
                ok(201, "{}")
            }
            ("GET", "/api/config/dns/hosts", _) => {
                ok(200, &format!(r#"{{"config":{{"dns":{{"hosts":[{}]}}}}}}"#, list(&st.hosts)))
            }
            ("GET", "/api/config/dns/cnameRecords", _) => {
                ok(200, &format!(r#"{{"config":{{"dns":{{"cnameRecords":[{}]}}}}}}"#, list(&st.cnames)))
            }
            (_, t, _) if t.starts_with("/api/queries") => ok(200, &st.queries.clone()),
            (m, _, p) => {
                let items = match p {
                    "/api/domains/deny/regex" => &mut st.regex,
                    "/api/config/dns/hosts" => &mut st.hosts,
                    "/api/config/dns/cnameRecords" => &mut st.cnames,
                    _ => return ok(404, r#"{"error":{"message":"Not found"}}"#),
                };
                match m {
                    "PUT" if items.contains(&entry) => {
                        ok(400, r#"{"error":{"key":"bad_request","message":"Item already present"}}"#)
                    }
                    "PUT" => {
                        items.push(entry);
                        ok(201, "{}")
                    }
                    "DELETE" if items.contains(&entry) => {
                        items.retain(|i| *i != entry);
                        ok(204, "")
                    }
                    _ => ok(404, r#"{"error":{"message":"Not found"}}"#),
                }
            }
        }
    }
}

fn rw(pattern: &str, answer: &str) -> Rewrite {
    Rewrite { pattern: pattern.into(), answer: answer.into() }
}

fn pihole(password: &str) -> (PiHole, Arc<Fake>) {
    let web = Arc::new(Fake::default());
    let at = Endpoint { host: "10.0.0.53".into(), port: 80, https: false, pin: None, unchecked: false };
    (PiHole::new(web.clone(), at, password), web)
}

#[test]
fn rewrites_to_an_address_are_reply_rules_and_every_session_ends() {
    let (p, web) = pihole("secret");
    web.0.lock().unwrap().hosts.push("10.0.0.5 printer.home".into());
    p.add(&rw("lancache.riotcdn.net", "10.0.0.210")).unwrap();
    p.add(&rw("*.dyn.riotcdn.net", "10.0.0.210")).unwrap();
    let got = p.rewrites().unwrap();
    assert_eq!(
        got,
        [
            rw("lancache.riotcdn.net", "10.0.0.210"),
            rw("*.dyn.riotcdn.net", "10.0.0.210"),
            rw("printer.home", "10.0.0.5")
        ]
    );
    p.remove(&rw("*.dyn.riotcdn.net", "10.0.0.210")).unwrap();
    let st = web.0.lock().unwrap();
    assert_eq!(st.regex, ["^lancache[.]riotcdn[.]net$;reply=10.0.0.210"]);
    assert!(st.sessions.is_empty(), "signed out after each operation");
    assert_eq!(p.kind(), "Pi-hole");
    assert!(!format!("{p:?}").contains("secret"), "no password in Debug");
}

#[test]
fn names_pointed_at_a_name_are_cnames_to_a_local_record_and_wildcards_take_its_address() {
    let (p, web) = pihole("secret");
    // As Apply does: the name first (a reply rule), then the names pointed at it.
    p.add(&rw("lanstash.lan", "10.0.0.210")).unwrap();
    p.add(&rw("lancache.riotcdn.net", "lanstash.lan")).unwrap();
    p.add(&rw("*.dyn.riotcdn.net", "lanstash.lan")).unwrap();
    {
        let st = web.0.lock().unwrap();
        assert_eq!(st.cnames, ["lancache.riotcdn.net,lanstash.lan"]);
        assert_eq!(st.hosts, ["10.0.0.210 lanstash.lan"], "a CNAME is followed only to a local record");
        assert_eq!(st.regex, ["[.]dyn[.]riotcdn[.]net$;reply=10.0.0.210"], "the target's reply rule went");
    }
    let got = p.rewrites().unwrap();
    assert!(
        got.contains(&rw("lanstash.lan", "10.0.0.210")) && got.contains(&rw("lancache.riotcdn.net", "lanstash.lan"))
    );
    let e = p.add(&rw("*.x.example", "nowhere.lan")).unwrap_err();
    assert!(e.contains("nowhere.lan has no address in Pi-hole"), "{e}");
    p.remove(&rw("lancache.riotcdn.net", "lanstash.lan")).unwrap();
    assert!(web.0.lock().unwrap().cnames.is_empty());
}

#[test]
fn a_name_is_taken_out_of_a_local_record_that_names_several() {
    let (p, web) = pihole("secret");
    web.0.lock().unwrap().hosts.push("10.0.0.210 a.example b.example".into());
    p.remove(&rw("a.example", "10.0.0.210")).unwrap();
    let st = web.0.lock().unwrap();
    assert_eq!(st.hosts, ["10.0.0.210 b.example"]);
    let changes: Vec<&String> = st.asked.iter().filter(|a| a.contains("/dns/hosts/")).collect();
    assert!(changes[0].starts_with("PUT") && changes[1].starts_with("DELETE"), "the rest kept first: {changes:?}");
}

#[test]
fn a_rule_of_the_users_own_is_left_alone() {
    let (p, web) = pihole("secret");
    // Written by hand, with an option of its own: it means the same, but it is not lanstash's.
    let own = "^lancache[.]riotcdn[.]net$;querytype=A;reply=10.0.0.210";
    web.0.lock().unwrap().regex.push(own.into());
    p.add(&rw("lancache.riotcdn.net", "10.0.0.210")).unwrap();
    p.remove(&rw("lancache.riotcdn.net", "10.0.0.210")).unwrap();
    assert_eq!(web.0.lock().unwrap().regex, [own], "only lanstash's own went");
}

#[test]
fn a_name_pointed_at_a_name_the_user_has_a_rule_for_answers_its_address_itself() {
    let (p, web) = pihole("secret");
    let own = "^lanstash[.]lan$;querytype=A;reply=10.0.0.210";
    web.0.lock().unwrap().regex.push(own.into());
    p.add(&rw("lancache.riotcdn.net", "lanstash.lan")).unwrap();
    let st = web.0.lock().unwrap();
    assert!(st.regex.contains(&own.to_owned()), "the user's rule stays");
    assert!(st.regex.contains(&"^lancache[.]riotcdn[.]net$;reply=10.0.0.210".to_owned()), "{:?}", st.regex);
    assert!(st.cnames.is_empty() && st.hosts.is_empty(), "no CNAME to a name a rule answers");
}

#[test]
fn reads_the_query_log_counting_reply_rules_and_local_records() {
    let (p, web) = pihole("secret");
    {
        let mut st = web.0.lock().unwrap();
        st.regex = vec!["^lancache[.]riotcdn[.]net$;reply=10.0.0.210".into(), "^ads[.]example$".into()];
        st.hosts = vec!["10.0.0.210 lanstash.lan".into()];
        st.queries = r#"{"queries":[
            {"id":4,"time":1793987676.2327294,"type":"A","status":"REGEX","domain":"lancache.riotcdn.net","upstream":null,"reply":{"type":"IP"},"client":{"ip":"10.0.1.100","name":"game-pc"},"list_id":1},
            {"id":3,"time":1793987675.5,"type":"A","status":"REGEX","domain":"ads.example","upstream":null,"reply":{"type":"IP"},"client":{"ip":"10.0.1.100"},"list_id":2},
            {"id":2,"time":1793987675.0,"type":"A","status":"CACHE","domain":"lanstash.lan","upstream":null,"reply":{"type":"IP"},"client":{"ip":"10.0.1.101"},"list_id":null},
            {"id":1,"time":1793987674.0,"type":"A","status":"FORWARDED","domain":"level3.blizzard.com","upstream":"8.8.8.8#53","reply":{"type":"IP"},"client":{"ip":"10.0.1.100"},"list_id":null}
        ]}"#
        .into();
    }
    let got = p.lookups(500).unwrap();
    let seen: Vec<(&str, bool)> = got.iter().map(|l| (l.name.as_str(), l.rewritten)).collect();
    assert_eq!(
        seen,
        [
            ("lancache.riotcdn.net", true),
            ("ads.example", false),
            ("lanstash.lan", true),
            ("level3.blizzard.com", false)
        ]
    );
    assert_eq!((got[0].client.as_str(), got[0].at_ms), ("10.0.1.100", 1_793_987_676_232));
    assert!(web.0.lock().unwrap().asked.iter().any(|a| a == "GET /api/queries?length=500"));
}

#[test]
fn says_why_it_failed() {
    let (p, web) = pihole("wrong");
    assert_eq!(p.rewrites().unwrap_err(), "Pi-hole refused lanstash's password");
    assert!(web.0.lock().unwrap().sessions.is_empty());
    let (p, web) = pihole("secret");
    p.add(&rw("x.example", "10.0.0.1")).unwrap();
    web.0.lock().unwrap().cnames.push("y.example,z.example".into());
    let e = p.remove(&rw("x.example", "10.0.0.1"));
    assert!(e.is_ok(), "{e:?}");
    // A CNAME already there is refused by Pi-hole, as it says.
    web.0.lock().unwrap().hosts.push("10.0.0.2 z.example".into());
    let e = p.add(&rw("y.example", "z.example")).unwrap_err();
    assert!(e.ends_with("Item already present"), "{e}");
}
