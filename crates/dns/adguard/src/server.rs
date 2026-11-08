//! AdGuard Home as the network's [`DnsServer`], through its admin API with
//! HTTP basic authentication (as of AdGuard Home v0.107.62):
//!   - `GET /control/rewrite/list`: `[{"domain","answer"}]`; a domain is
//!     `name` (that name only) or `*.name` (its subdomains only), as in the
//!     cache-domains lists, so a pattern is a domain as it is;
//!   - `POST /control/rewrite/add` and `/control/rewrite/delete` with
//!     `{"domain","answer"}`: adding one that is there makes a second copy,
//!     deleting removes every copy; both answer 200 (a missing one too);
//!   - `GET /control/querylog?limit=N`: `{"data":[...],"oldest"}`, newest
//!     first; an entry has `question.name`, `client`, `reason` (`Rewrite`
//!     when a rewrite answered, `RewriteRule` a `$dnsrewrite` rule,
//!     `FilteredBlackList` a blocking rule, hosts-file rules among them, with
//!     the hosts rule's address answered), `answer[]` (`type`, `value`) and `time`;
//!   - `GET /control/filtering/check_host?name=`: how it answers a name ([`crate::check`]).
//!
//! A request without the right user and password gets 401 or 403. The admin
//! API is plain HTTP or HTTPS ([`Endpoint`]); over plain HTTP an HTTPS-only
//! AdGuard answers 400 ("Client sent an HTTP request to an HTTPS server").

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use lanstash_domain::checksum::sha1::base64;
use lanstash_domain::json::{Value, parse, string};
use lanstash_ports::dns::{Answered, DnsServer, Lookup, Rewrite};
use lanstash_ports::web::{Endpoint, Web, WebRequest};

use crate::{check, time};

/// Where AdGuard Home's admin API is, and who signs in.
pub struct AdGuard {
    web: Arc<dyn Web>,
    at: Endpoint,
    authorization: String,
}

impl core::fmt::Debug for AdGuard {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never the credentials.
        f.debug_struct("AdGuard").field("at", &self.at).finish_non_exhaustive()
    }
}

impl AdGuard {
    #[must_use]
    pub fn new(web: Arc<dyn Web>, at: Endpoint, user: &str, password: &str) -> Self {
        let authorization = format!("Basic {}", base64(format!("{user}:{password}").as_bytes()));
        Self { web, at, authorization }
    }

    /// Sends a request; the body of a 200, else why not.
    fn call(&self, method: &str, target: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        let headers = [("Authorization", self.authorization.as_str()), ("Content-Type", "application/json")];
        let tls = self.at.tls();
        let (host, port) = (self.at.host.as_str(), self.at.port);
        let req = WebRequest { method, host, port, target, headers: &headers, body, tls };
        // The error's words keep an untrusted certificate's SHA-256 (`lanstash_ports::web::untrusted_in`).
        let r = self.web.send(&req).map_err(|e| format!("AdGuard Home at {host}:{port}: {e}"))?;
        let why = String::from_utf8_lossy(&r.body);
        // Plain HTTP to an HTTPS-only admin page: a 400 saying so, or a redirect to https://.
        let https_only =
            (r.status == 400 && why.contains("HTTPS")) || ((300..400).contains(&r.status) && why.contains("https://"));
        match r.status {
            200 => Ok(r.body),
            _ if https_only && !self.at.https => {
                Err(format!("AdGuard Home at {host}:{port} is HTTPS-only: choose HTTPS (and its HTTPS port)"))
            }
            401 | 403 => Err("AdGuard Home refused lanstash's user name and password".into()),
            s => Err(format!("AdGuard Home answered {s} to {method} {target}: {}", why.trim())),
        }
    }

    fn json(&self, target: &str) -> Result<Value, String> {
        let body = self.call("GET", target, b"")?;
        parse(&String::from_utf8_lossy(&body)).map_err(|e| format!("AdGuard Home's {target}: {e:?}"))
    }

    fn post(&self, target: &str, r: &Rewrite) -> Result<(), String> {
        let body = format!("{{\"domain\":{},\"answer\":{}}}", string(&r.pattern), string(&r.answer));
        self.call("POST", target, body.as_bytes()).map(|_| ())
    }
}

fn text(v: Option<&Value>) -> String {
    v.and_then(Value::as_str).unwrap_or_default().to_string()
}

/// One query log entry as a lookup; none without a name.
fn lookup(e: &Value) -> Option<Lookup> {
    let name = e.get("question")?.get("name")?.as_str()?.trim_end_matches('.').to_string();
    let answers = e
        .get("answer")
        .map(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter(|a| matches!(a.get("type").and_then(Value::as_str), Some("A" | "AAAA")))
        .filter_map(|a| a.get("value")?.as_str().map(ToString::to_string))
        .collect::<Vec<String>>();
    // Answered by AdGuard itself: a rewrite or rewrite rule, or a hosts rule's real address.
    let reason = e.get("reason").and_then(Value::as_str).unwrap_or_default();
    let hosts = reason == "FilteredBlackList" && answers.iter().any(|a| !check::unspecified(a));
    Some(Lookup {
        name,
        client: text(e.get("client")),
        rewritten: reason.starts_with("Rewrite") || hosts,
        answers,
        at_ms: e.get("time").and_then(Value::as_str).and_then(time::ms).unwrap_or(0),
    })
}

impl DnsServer for AdGuard {
    fn kind(&self) -> &str {
        "AdGuard Home"
    }

    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        Ok(self
            .json("/control/rewrite/list")?
            .as_array()
            .iter()
            .map(|r| Rewrite { pattern: text(r.get("domain")), answer: text(r.get("answer")) })
            .filter(|r| !r.pattern.is_empty())
            .collect())
    }

    fn add(&self, rewrite: &Rewrite) -> Result<(), String> {
        self.post("/control/rewrite/add", rewrite)
    }

    fn remove(&self, rewrite: &Rewrite) -> Result<(), String> {
        self.post("/control/rewrite/delete", rewrite)
    }

    fn lookups(&self, limit: usize) -> Result<Vec<Lookup>, String> {
        let log = self.json(&format!("/control/querylog?limit={limit}"))?;
        Ok(log.get("data").map(Value::as_array).unwrap_or_default().iter().filter_map(lookup).collect())
    }

    fn check(&self, name: &str) -> Option<Result<Answered, String>> {
        // A host name needs no escaping; anything else is not asked about.
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return Some(Err(format!("{name} is not a host name")));
        }
        Some(self.json(&format!("/control/filtering/check_host?name={name}")).map(|v| check::answered(&v)))
    }
}
