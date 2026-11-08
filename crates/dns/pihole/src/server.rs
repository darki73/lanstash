//! Pi-hole as the network's [`DnsServer`], through its v6 API (as of
//! Pi-hole 2026.10.0):
//!   - `POST /api/auth` with `{"password"}`: `{"session":{"valid","sid"}}`
//!     (401 for a wrong password; `sid` null when Pi-hole has none), the
//!     session sent as `X-FTL-SID`; `DELETE /api/auth` ends it. Pi-hole keeps
//!     few sessions at once, so each operation signs in and out again;
//!   - `GET|POST /api/domains/deny/regex` (`{"domain","comment"}`, 201) and
//!     `DELETE /api/domains/deny/regex/<rule>` (204): the reply rules lanstash
//!     writes ([`crate::rules`]);
//!   - `GET /api/config/dns/hosts` and `.../cnameRecords` (`{"config":{"dns":{...}}}`),
//!     `PUT|DELETE /api/config/dns/<list>/<entry>` (201, 204; adding an entry that is
//!     there is refused with 400 "Item already present", so none is there twice);
//!   - `GET /api/queries?length=N`: newest first; `domain`, `client.ip`, `time`
//!     (seconds), `status` (`REGEX` for a regex rule, with `list_id` its id;
//!     `CACHE` with no `upstream` for a local record, as for a cached answer:
//!     told apart by the name having a local record), no answered addresses.
//!
//! `misc.dnsmasq_lines` (where `address=/name/IP` would go) cannot be set through
//! the API, so wildcards are reply rules. Changes answer a few seconds later
//! (Pi-hole reloads its resolver).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use lanstash_domain::json::{Value, parse, string};
use lanstash_ports::dns::{DnsServer, Lookup, Rewrite};
use lanstash_ports::web::{Endpoint, Web, WebRequest};

use crate::rules::{self, encode, from_cname, from_host, from_regex, is_address};

/// Where Pi-hole's API is, and its password.
pub struct PiHole {
    web: Arc<dyn Web>,
    at: Endpoint,
    password: String,
}

impl core::fmt::Debug for PiHole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never the password.
        f.debug_struct("PiHole").field("at", &self.at).finish_non_exhaustive()
    }
}

/// The comment on the rules lanstash writes.
const COMMENT: &str = "lanstash";
const REGEX: &str = "/api/domains/deny/regex";
const HOSTS: &str = "/api/config/dns/hosts";
const CNAMES: &str = "/api/config/dns/cnameRecords";

/// One regex rule of Pi-hole's.
struct Regex {
    id: u64,
    rule: String,
    enabled: bool,
    /// lanstash wrote it (its comment says so).
    ours: bool,
}

impl Regex {
    /// The rewrite it is, when it is one in effect.
    fn rewrite(&self) -> Option<Rewrite> {
        self.enabled.then(|| from_regex(&self.rule)).flatten()
    }

    /// Whether lanstash may take it away for `rewrite`: in effect for it, and lanstash's own
    /// (or written exactly as lanstash writes it; one with options of its own is the user's).
    fn ours_for(&self, rewrite: &Rewrite) -> bool {
        self.rewrite().as_ref() == Some(rewrite)
            && (self.ours || rules::regex(&rewrite.pattern, &rewrite.answer).is_ok_and(|r| r == self.rule))
    }
}

/// A signed-in session (none when Pi-hole has no password).
struct Session<'a> {
    pi: &'a PiHole,
    sid: Option<String>,
}

impl PiHole {
    #[must_use]
    pub fn new(web: Arc<dyn Web>, at: Endpoint, password: &str) -> Self {
        Self { web, at, password: password.to_string() }
    }

    /// Sends a request; the status and body of a 2xx, else why not.
    fn send(&self, method: &str, target: &str, body: &[u8], sid: Option<&str>) -> Result<(u16, Vec<u8>), String> {
        let mut headers = Vec::from([("Content-Type", "application/json")]);
        if let Some(s) = sid {
            headers.push(("X-FTL-SID", s));
        }
        let (host, port) = (self.at.host.as_str(), self.at.port);
        let req = WebRequest { method, host, port, target, headers: &headers, body, tls: self.at.tls() };
        // The error's words keep an untrusted certificate's SHA-256 (`lanstash_ports::web::untrusted_in`).
        let r = self.web.send(&req).map_err(|e| format!("Pi-hole at {host}:{port}: {e}"))?;
        let why = String::from_utf8_lossy(&r.body);
        // Its errors: {"error":{"key","message","hint"}}.
        let message = parse(&why).ok().and_then(|v| v.get("error")?.get("message")?.as_str().map(ToString::to_string));
        match r.status {
            200..=299 => Ok((r.status, r.body)),
            401 | 403 => Err("Pi-hole refused lanstash's password".into()),
            s => Err(format!(
                "Pi-hole answered {s} to {method} {target}: {}",
                message.unwrap_or_else(|| why.trim().into())
            )),
        }
    }

    /// Signs in, runs `f`, signs out (whatever `f` gave).
    fn session<T>(&self, f: impl FnOnce(&Session<'_>) -> Result<T, String>) -> Result<T, String> {
        let body = format!("{{\"password\":{}}}", string(&self.password));
        let (_, answer) = self.send("POST", "/api/auth", body.as_bytes(), None)?;
        let v = parse(&String::from_utf8_lossy(&answer)).map_err(|e| format!("Pi-hole's /api/auth: {e:?}"))?;
        let session = v.get("session");
        if !matches!(session.and_then(|s| s.get("valid")), Some(Value::Bool(true))) {
            return Err("Pi-hole refused lanstash's password".into());
        }
        let sid = session.and_then(|s| s.get("sid")).and_then(Value::as_str).map(ToString::to_string);
        let s = Session { pi: self, sid };
        let out = f(&s);
        if let Some(sid) = &s.sid {
            let _ = self.send("DELETE", "/api/auth", b"", Some(sid));
        }
        out
    }
}

impl Session<'_> {
    fn call(&self, method: &str, target: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        self.pi.send(method, target, body, self.sid.as_deref()).map(|(_, b)| b)
    }

    fn json(&self, target: &str) -> Result<Value, String> {
        let body = self.call("GET", target, b"")?;
        parse(&String::from_utf8_lossy(&body)).map_err(|e| format!("Pi-hole's {target}: {e:?}"))
    }

    /// The entries of a local DNS list (`hosts`, `cnameRecords`).
    fn list(&self, target: &str, key: &str) -> Result<Vec<String>, String> {
        let v = self.json(target)?;
        let items = v.get("config").and_then(|c| c.get("dns")).and_then(|d| d.get(key));
        Ok(items
            .map(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|i| i.as_str().map(ToString::to_string))
            .collect())
    }

    /// Adds the reply rule of `pattern` to `address`.
    fn rule(&self, pattern: &str, address: &str) -> Result<(), String> {
        let rule = rules::regex(pattern, address)?;
        let body = format!("{{\"domain\":{},\"comment\":{}}}", string(&rule), string(COMMENT));
        self.call("POST", REGEX, body.as_bytes()).map(|_| ())
    }

    /// The regex rules.
    fn regexes(&self) -> Result<Vec<Regex>, String> {
        let v = self.json(REGEX)?;
        Ok(v.get("domains")
            .map(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|d| {
                let id = match d.get("id") {
                    Some(Value::Number(n)) => *n as u64,
                    _ => 0,
                };
                let enabled = !matches!(d.get("enabled"), Some(Value::Bool(false)));
                let ours = d.get("comment").and_then(Value::as_str) == Some(COMMENT);
                Some(Regex { id, rule: d.get("domain")?.as_str()?.to_string(), enabled, ours })
            })
            .collect())
    }
}

/// What `entry` of a local list is once `name` is taken out of it (a hosts line names several).
fn without(line: &str, name: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    let address = words.next()?;
    let rest: Vec<&str> = words.filter(|w| !w.eq_ignore_ascii_case(name)).collect();
    (!rest.is_empty()).then(|| format!("{address} {}", rest.join(" ")))
}

impl DnsServer for PiHole {
    fn kind(&self) -> &str {
        "Pi-hole"
    }

    fn rewrites(&self) -> Result<Vec<Rewrite>, String> {
        self.session(|s| {
            let mut out: Vec<Rewrite> = s.regexes()?.iter().filter_map(Regex::rewrite).collect();
            out.extend(s.list(HOSTS, "hosts")?.iter().flat_map(|l| from_host(l)));
            out.extend(s.list(CNAMES, "cnameRecords")?.iter().filter_map(|e| from_cname(e)));
            Ok(out)
        })
    }

    fn add(&self, rewrite: &Rewrite) -> Result<(), String> {
        self.session(|s| {
            if is_address(&rewrite.answer) {
                return s.rule(&rewrite.pattern, &rewrite.answer);
            }
            // Pointed at a name: the address that name has in Pi-hole's own records.
            let hosts = s.list(HOSTS, "hosts")?;
            let local =
                hosts.iter().flat_map(|l| from_host(l)).find(|r| r.pattern.eq_ignore_ascii_case(&rewrite.answer));
            let ruled = || -> Result<Option<String>, String> {
                let own = s.regexes()?;
                let found =
                    own.iter().filter_map(Regex::rewrite).find(|r| r.pattern.eq_ignore_ascii_case(&rewrite.answer));
                Ok(found.map(|r| r.answer))
            };
            let address = match &local {
                Some(r) => Some(r.answer.clone()),
                None => ruled()?,
            };
            let Some(address) = address else {
                return Err(format!("{} has no address in Pi-hole: add one for it first", rewrite.answer));
            };
            // A wildcard cannot be a CNAME: it answers that address itself.
            if rewrite.pattern.starts_with("*.") {
                return s.rule(&rewrite.pattern, &address);
            }
            // A CNAME is followed only to a name with a local record and no deny rule (Pi-hole inspects
            // CNAME targets, a reply rule among them): the target moves from its reply rule to a record.
            // A reply rule of the user's own for it stays: then this name answers the address itself.
            let target = Rewrite { pattern: rewrite.answer.to_ascii_lowercase(), answer: address.clone() };
            let regexes = s.regexes()?;
            let targets: Vec<&Regex> = regexes.iter().filter(|r| r.rewrite().as_ref() == Some(&target)).collect();
            if targets.iter().any(|r| !r.ours_for(&target)) {
                return s.rule(&rewrite.pattern, &address);
            }
            if local.is_none() {
                let line = format!("{address} {}", rewrite.answer);
                s.call("PUT", &format!("{HOSTS}/{}", encode(&line)), b"")?;
            }
            for r in targets {
                s.call("DELETE", &format!("{REGEX}/{}", encode(&r.rule)), b"")?;
            }
            let entry = format!("{},{}", rewrite.pattern, rewrite.answer);
            s.call("PUT", &format!("{CNAMES}/{}", encode(&entry)), b"").map(|_| ())
        })
    }

    fn remove(&self, rewrite: &Rewrite) -> Result<(), String> {
        self.session(|s| {
            // Whichever way it is written: reply rules (lanstash's), local records, CNAMEs.
            for r in s.regexes()?.iter().filter(|r| r.ours_for(rewrite)) {
                s.call("DELETE", &format!("{REGEX}/{}", encode(&r.rule)), b"")?;
            }
            for line in s.list(HOSTS, "hosts")?.iter().filter(|l| from_host(l).contains(rewrite)) {
                // The line's other names first, so a failure loses none of them.
                if let Some(rest) = without(line, &rewrite.pattern) {
                    s.call("PUT", &format!("{HOSTS}/{}", encode(&rest)), b"")?;
                }
                s.call("DELETE", &format!("{HOSTS}/{}", encode(line)), b"")?;
            }
            for entry in s.list(CNAMES, "cnameRecords")?.iter().filter(|e| from_cname(e).as_ref() == Some(rewrite)) {
                s.call("DELETE", &format!("{CNAMES}/{}", encode(entry)), b"")?;
            }
            Ok(())
        })
    }

    fn lookups(&self, limit: usize) -> Result<Vec<Lookup>, String> {
        self.session(|s| {
            let log = s.json(&format!("/api/queries?length={limit}"))?;
            // A regex rule answers for lanstash when it replies with an address; a local record always does.
            let replying: Vec<u64> = s
                .regexes()?
                .iter()
                .filter(|r| from_regex(&r.rule).is_some_and(|w| w.answer != "0.0.0.0" && w.answer != "::"))
                .map(|r| r.id)
                .collect();
            let mut local: Vec<String> =
                s.list(HOSTS, "hosts")?.iter().flat_map(|l| from_host(l)).map(|r| r.pattern).collect();
            local.extend(s.list(CNAMES, "cnameRecords")?.iter().filter_map(|e| from_cname(e)).map(|r| r.pattern));
            let queries = log.get("queries").map(Value::as_array).unwrap_or_default();
            Ok(queries
                .iter()
                .filter_map(|q| {
                    let name = q.get("domain")?.as_str()?.trim_end_matches('.').to_string();
                    let status = q.get("status").and_then(Value::as_str).unwrap_or_default();
                    let list = match q.get("list_id") {
                        Some(Value::Number(n)) => Some(*n as u64),
                        _ => None,
                    };
                    let upstream = q.get("upstream").and_then(Value::as_str).is_some();
                    let rewritten = (status == "REGEX" && list.is_some_and(|l| replying.contains(&l)))
                        || (status == "CACHE" && !upstream && local.iter().any(|n| n.eq_ignore_ascii_case(&name)));
                    let at_ms = match q.get("time") {
                        Some(Value::Number(t)) if *t > 0.0 => (*t * 1000.0) as u64,
                        _ => 0,
                    };
                    let client = q.get("client").and_then(|c| c.get("ip")).and_then(Value::as_str).unwrap_or_default();
                    Some(Lookup { name, client: client.to_string(), answers: Vec::new(), rewritten, at_ms })
                })
                .collect())
        })
    }
}
