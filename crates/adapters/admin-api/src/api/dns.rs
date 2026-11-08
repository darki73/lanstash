//! The DNS endpoints, for the web UI's DNS page:
//! - `GET /api/v1/dns`: the setup (the kinds of server lanstash knows, the
//!   server set up, whether a password is kept; never the password) and, once
//!   one is set up, the report: its rewrites against the names lanstash wants
//!   (`covered`: names it sends to lanstash anyway, by the rule that does),
//!   the names that went to the internet, the hosts lanstash refused;
//! - `PUT /api/v1/dns/setup?kind=&host=&port=&user=[&answer=][&https=1[&pin=|&unchecked=1]][&test=1]`,
//!   the password in the `X-Lanstash-Password` header (none: the kept one):
//!   checks and tries the setup, keeps it unless testing (400 with why when it
//!   does not work; `kind` `untrusted` with the `certificate`'s SHA-256 when
//!   HTTPS did not trust it); then the same as `GET`, with what trying found;
//! - `DELETE /api/v1/dns/setup`: forgets the setup and its password;
//! - `POST /api/v1/dns/apply`: adds the missing rewrites, leaves one of each
//!   duplicate (502 when the server could not be asked).
//!
//! Without the DNS link wired every path answers 404.

use lanstash_app::dns::setup::Tried;
use lanstash_app::dns::{AnswerFrom, Applied, DnsLink, Report};
use lanstash_domain::json::string;
use lanstash_ports::dns::DnsConfig;
use lanstash_ports::web::untrusted_in;

use super::settings::decode;
use super::{AdminApi, Answer};
use crate::route::Route;

/// The request header the password comes in (lower case, as the server gives header names).
pub const PASSWORD_HEADER: &str = "x-lanstash-password";

fn list(items: &[String]) -> String {
    let v: Vec<String> = items.iter().map(|s| string(s)).collect();
    format!("[{}]", v.join(","))
}

fn opt(v: Option<&str>) -> String {
    v.map_or_else(|| "null".to_owned(), string)
}

/// The setup as JSON: never the password, only whether one is kept.
fn render_setup(link: &DnsLink) -> String {
    let s = link.setup();
    let kinds: Vec<String> = s
        .kinds()
        .iter()
        .map(|k| {
            format!(
                "{{\"id\":{},\"name\":{},\"port\":{},\"https_port\":{},\"user\":{}}}",
                string(&k.id),
                string(&k.name),
                k.port,
                k.https_port,
                k.user
            )
        })
        .collect();
    let config = s.config().map_or_else(
        || "null".to_owned(),
        |c| {
            format!(
                "{{\"kind\":{},\"host\":{},\"port\":{},\"https\":{},\"pin\":{},\"unchecked\":{},\"user\":{},\"answer\":{},\"via\":{}}}",
                string(&c.kind),
                string(&c.host),
                c.port,
                c.https,
                opt(c.pin.as_deref()),
                c.unchecked,
                string(&c.user),
                opt(c.answer.as_deref()),
                opt(c.via.as_deref())
            )
        },
    );
    format!(
        "{{\"kinds\":[{}],\"config\":{config},\"password_set\":{}}}",
        kinds.join(","),
        s.password().is_some_and(|p| !p.is_empty())
    )
}

/// The report as JSON.
#[must_use]
pub fn render(r: &Report) -> String {
    let (plan, plan_error) = match &r.plan {
        Ok(p) => (
            format!(
                "{{\"answer\":{},\"write_as\":{},\"via\":{},\"via_missing\":{},\"services\":{{{}}},\"answer_from\":{},\"wanted\":{},\"missing\":{},\"covered\":{{{}}},\"duplicates\":{},\"extra\":{}}}",
                string(&p.answer),
                string(&p.write_as),
                opt(p.via.as_deref()),
                p.via_missing,
                p.services.iter().map(|(n, s)| format!("{}:{}", string(n), string(s))).collect::<Vec<_>>().join(","),
                string(match p.answer_from {
                    AnswerFrom::Config => "config",
                    AnswerFrom::Rewrites => "rewrites",
                }),
                p.wanted,
                list(&p.missing),
                p.covered.iter().map(|(n, by)| format!("{}:{}", string(n), string(by))).collect::<Vec<_>>().join(","),
                list(&p.duplicates),
                list(&p.extra),
            ),
            "null".to_owned(),
        ),
        Err(e) => ("null".to_owned(), string(e)),
    };
    let missed: Vec<String> = r
        .missed
        .iter()
        .map(|m| {
            format!(
                "{{\"name\":{},\"service\":{},\"count\":{},\"clients\":{}}}",
                string(&m.name),
                string(&m.service),
                m.count,
                list(&m.clients)
            )
        })
        .collect();
    let refused: Vec<String> =
        r.refused.iter().map(|(h, n)| format!("{{\"host\":{},\"count\":{n}}}", string(h))).collect();
    format!(
        "{{\"server\":{},\"plan\":{plan},\"plan_error\":{plan_error},\"missed\":[{}],\"lookups\":{},\"oldest_ms\":{},\"lookups_error\":{},\"refused\":[{}],\"refused_others\":{}}}",
        string(&r.kind),
        missed.join(","),
        r.lookups,
        r.oldest_ms.map_or_else(|| "null".to_owned(), |t| t.to_string()),
        opt(r.lookups_error.as_deref()),
        refused.join(","),
        r.refused_others,
    )
}

/// Everything the page shows, and what was just done (`done`: a JSON object, or none).
fn page(link: &DnsLink, done: Option<String>) -> String {
    let (report, error) = match link.report() {
        Ok(r) => (render(&r), "null".to_owned()),
        Err(e) => ("null".to_owned(), string(&e)),
    };
    format!(
        "{{\"setup\":{},\"report\":{report},\"report_error\":{error},\"done\":{}}}\n",
        render_setup(link),
        done.unwrap_or_else(|| "null".to_owned())
    )
}

/// The value of `key` in `target`'s query, decoded.
pub(super) fn query(target: &str, key: &str) -> Option<String> {
    let (_, q) = target.split_once('?')?;
    q.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('=')).and_then(decode)
}

impl AdminApi {
    /// Enables the DNS paths.
    #[must_use]
    pub fn with_dns(mut self, dns: std::sync::Arc<DnsLink>) -> Self {
        self.dns = Some(dns);
        self
    }

    /// `GET /api/v1/dns` and `POST /api/v1/dns/apply`.
    pub(super) fn dns(&self, route: Route, target: &str) -> Answer {
        let Some(link) = &self.dns else { return Answer::error(target, 404, "not-found", "not found") };
        if route == Route::DnsApply {
            return match link.apply() {
                Ok(Applied { added, deduplicated }) => {
                    let done = format!("{{\"added\":{added},\"deduplicated\":{deduplicated}}}");
                    Answer::json(200, page(link, Some(done)))
                }
                Err(e) => Answer::error(target, 502, "unavailable", &e),
            };
        }
        Answer::json(200, page(link, None))
    }

    /// `PUT` and `DELETE /api/v1/dns/setup`; `password` from its header.
    pub(super) fn dns_setup(&self, route: Route, target: &str, password: Option<&str>) -> Answer {
        let Some(link) = &self.dns else { return Answer::error(target, 404, "not-found", "not found") };
        if route == Route::DnsForget {
            return match link.forget() {
                Ok(()) => Answer::json(200, page(link, None)),
                Err(e) => Answer::error(target, 500, "failed", &e),
            };
        }
        let flag = |key: &str| query(target, key).is_some_and(|v| v == "1");
        let config = DnsConfig {
            kind: query(target, "kind").unwrap_or_default(),
            host: query(target, "host").unwrap_or_default(),
            port: query(target, "port").and_then(|p| p.trim().parse().ok()).unwrap_or(0),
            https: flag("https"),
            pin: query(target, "pin"),
            unchecked: flag("unchecked"),
            user: query(target, "user").unwrap_or_default(),
            answer: query(target, "answer"),
            via: query(target, "via"),
        };
        let test = query(target, "test").is_some_and(|t| t == "1");
        match link.configure(&config, password, !test) {
            Ok(Tried { kind, rewrites }) => {
                let done = format!(
                    "{{\"tested\":{test},\"kept\":{},\"server\":{},\"rewrites\":{rewrites}}}",
                    !test,
                    string(&kind)
                );
                Answer::json(200, page(link, Some(done)))
            }
            // An untrusted certificate: its SHA-256 too, so the page can offer to trust it.
            Err(e) => match untrusted_in(&e) {
                Some(fp) => Answer::json(
                    400,
                    format!("{{\"error\":{},\"kind\":\"untrusted\",\"certificate\":{}}}\n", string(&e), string(fp)),
                ),
                None => Answer::error(target, 400, "invalid", &e),
            },
        }
    }
}
