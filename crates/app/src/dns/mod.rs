//! Use case: lanstash and the network's DNS server ([`DnsServer`]: AdGuard
//! Home, Pi-hole, ...), which sends the game CDN names to the cache. Which
//! server it is and how lanstash signs in is set up in the web UI ([`setup`]);
//! each request asks the server as set up then, so a change applies at once.
//!
//! - The rewrites lanstash wants: one per pattern of the cached services' host
//!   lists (as written: `name` or `*.name`), answered with lanstash's address:
//!   the one set up, else the address the server's rewrites of those
//!   patterns already answer most. The plan says which are missing, which are
//!   there twice, and which rewrites answer lanstash's address for names it
//!   does not want (reported only). A pattern with no rewrite that the server
//!   sends to lanstash anyway (a rule, a filter list) is covered ([`covered`]).
//! - Applying adds the missing ones and leaves one of each duplicate. It never
//!   removes a rewrite lanstash does not want: one for another address, or one
//!   that points at lanstash's host for something else, stays as it is.
//! - Coverage: the server's latest lookups of names a cached service owns that
//!   no rewrite answered (they went to the internet), by name, with how often
//!   and by whom; and the hosts lanstash itself refused since its start (a
//!   name the server sends to the cache but no host list names).

pub mod covered;
pub mod setup;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::dns::{DnsConfig, DnsServer, DnsSetup, Rewrite};

use crate::serve::Cache;

/// How many lookups a report reads.
pub const LOOKUPS: usize = 5_000;

/// Where lanstash's address came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerFrom {
    /// The configuration (`dns_answer`).
    Config,
    /// The server's rewrites of lanstash's patterns.
    Rewrites,
}

/// What the server's rewrites lack, against what lanstash wants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// lanstash's address.
    pub answer: String,
    pub answer_from: AnswerFrom,
    /// What new rewrites answer: a name of lanstash's host when the existing ones answer with it, else the address.
    pub write_as: String,
    /// The name the names are pointed at, when set up so.
    pub via: Option<String>,
    /// That name has no rewrite yet: Apply adds one to lanstash's address first.
    pub via_missing: bool,
    /// Patterns wanted.
    pub wanted: usize,
    /// Every pattern wanted, with the service its list is (in the order wanted).
    pub services: Vec<(String, String)>,
    /// Patterns with no rewrite to lanstash's address yet.
    pub missing: Vec<String>,
    /// Patterns with no rewrite of their own that the server sends to lanstash anyway, with
    /// the rule that does (or [`covered::BY_REWRITE`]); Apply adds nothing for them.
    pub covered: Vec<(String, String)>,
    /// Patterns whose rewrite is there more than once.
    pub duplicates: Vec<String>,
    /// Patterns answered with lanstash's address that lanstash does not want (left alone).
    pub extra: Vec<String>,
}

/// Lookups of a name a cached service owns that no rewrite answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missed {
    pub name: String,
    pub service: String,
    pub count: u64,
    /// The clients that looked it up, at most a few.
    pub clients: Vec<String>,
}

/// Everything the DNS page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The server, in words.
    pub kind: String,
    /// The plan, or why there is none.
    pub plan: Result<Plan, String>,
    /// Names that went to the internet, most looked up first.
    pub missed: Vec<Missed>,
    /// Lookups read, and the oldest one's time.
    pub lookups: usize,
    pub oldest_ms: Option<u64>,
    /// Why the lookups could not be read.
    pub lookups_error: Option<String>,
    /// Hosts lanstash refused since its start, most refused first, and the refusals of hosts beyond them.
    pub refused: Vec<(String, u64)>,
    pub refused_others: u64,
}

/// What applying the plan did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub added: usize,
    pub deduplicated: usize,
}

/// lanstash and its DNS server, as set up in the web UI.
pub struct DnsLink {
    setup: Arc<dyn DnsSetup>,
    cache: Arc<Cache>,
    hosts: HostRules,
    wanted: Vec<String>,
}

impl std::fmt::Debug for DnsLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DnsLink").field("wanted", &self.wanted.len()).finish_non_exhaustive()
    }
}

/// Why there is no server to ask.
pub const NOT_SET_UP: &str = "no DNS server is set up yet";

/// Whether `a` reads as an IP address (IPv4 digits and dots, or IPv6 hex and colons).
fn is_address(a: &str) -> bool {
    let v4 = a.split('.').count() == 4 && a.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    v4 || (a.contains(':') && a.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.'))
}

/// Whether `pattern` (`name` or `*.name`) covers `name` the way the server matches it.
fn covers(pattern: &str, name: &str) -> bool {
    match pattern.strip_prefix("*.") {
        // On bytes: a name the server returns may hold any text, not only ASCII.
        Some(base) => {
            let (name, base) = (name.as_bytes(), base.as_bytes());
            name.len() > base.len() + 1
                && name[name.len() - base.len()..].eq_ignore_ascii_case(base)
                && name[name.len() - base.len() - 1] == b'.'
        }
        None => pattern.eq_ignore_ascii_case(name),
    }
}

/// What a rewrite's `answer` ends up as: itself when it is an address, else what the rewrites give
/// that name (an exact rewrite before a wildcard one, the longest wildcard first, as AdGuard Home
/// matches), followed a few steps (`dist.blizzard.com` → `cache.lan` → `10.0.0.10`).
fn resolve<'a>(current: &'a [Rewrite], answer: &'a str) -> &'a str {
    let mut a = answer.trim_end_matches('.');
    for _ in 0..8 {
        if is_address(a) {
            return a;
        }
        let exact = current.iter().find(|r| !r.pattern.starts_with("*.") && covers(&r.pattern, a));
        let wild = || {
            current
                .iter()
                .filter(|r| r.pattern.starts_with("*.") && covers(&r.pattern, a))
                .max_by_key(|r| r.pattern.len())
        };
        match exact.or_else(wild) {
            Some(r) => a = r.answer.trim_end_matches('.'),
            None => return a,
        }
    }
    a
}

impl DnsLink {
    /// `wanted`: the patterns lanstash wants rewritten (duplicates dropped); `hosts`: which
    /// names a cached service owns; `setup`: which server, where, and how to sign in.
    #[must_use]
    pub fn new(setup: Arc<dyn DnsSetup>, cache: Arc<Cache>, hosts: HostRules, wanted: Vec<String>) -> Self {
        let wanted: Vec<String> = wanted.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
        Self { setup, cache, hosts, wanted }
    }

    /// The setup.
    #[must_use]
    pub fn setup(&self) -> &dyn DnsSetup {
        self.setup.as_ref()
    }

    /// The server as set up, and its setup.
    ///
    /// # Errors
    /// None set up, or of a kind lanstash does not know.
    pub fn server(&self) -> Result<(Arc<dyn DnsServer>, DnsConfig), String> {
        let config = self.setup.config().ok_or(NOT_SET_UP)?;
        let password = self.setup.password().unwrap_or_default();
        let server =
            self.setup.connect(&config, &password).ok_or_else(|| format!("unknown DNS server {}", config.kind))?;
        Ok((server, config))
    }

    /// The plan against `current` rewrites, with lanstash's address `answer` when it is set.
    ///
    /// # Errors
    /// No address set and none to infer.
    pub fn plan_for(&self, current: &[Rewrite], answer: Option<&str>, via: Option<&str>) -> Result<Plan, String> {
        // Pointed through a name that already leads to an address: that address is lanstash's, unless set.
        let through = via.map(|v| resolve(current, v)).filter(|a| is_address(a));
        let answer = answer.or(through);
        // Set up as a name: the rewrites answer with it, and lanstash's address is where it leads.
        let named = answer.filter(|a| !is_address(a));
        if let Some(name) = named {
            let to = resolve(current, name);
            if !is_address(to) {
                return Err(format!(
                    "{name} does not lead to an address in the rewrites: add a rewrite of it to lanstash's address"
                ));
            }
        }
        let (answer, answer_from) = match answer {
            Some(a) => (resolve(current, a).to_owned(), AnswerFrom::Config),
            None => {
                let mut votes: BTreeMap<&str, usize> = BTreeMap::new();
                for r in current.iter().filter(|r| self.wanted.contains(&r.pattern)) {
                    *votes.entry(resolve(current, &r.answer)).or_default() += 1;
                }
                let best = votes.into_iter().max_by_key(|(_, n)| *n).map(|(a, _)| a.to_owned());
                let why = "no address for lanstash: none of its names has a rewrite yet; set lanstash's address";
                (best.ok_or(why)?, AnswerFrom::Rewrites)
            }
        };
        // Lanstash's rewrites: those whose answer ends up as its address, through names too.
        let ours_list: Vec<&Rewrite> = current.iter().filter(|r| resolve(current, &r.answer) == answer).collect();
        let mut ours: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &ours_list {
            *ours.entry(r.pattern.as_str()).or_default() += 1;
        }
        // What new rewrites answer: the way the existing ones of lanstash's names do (a host name), else the address.
        let mut ways: BTreeMap<&str, usize> = BTreeMap::new();
        for r in ours_list.iter().filter(|r| self.wanted.contains(&r.pattern)) {
            *ways.entry(r.answer.as_str()).or_default() += 1;
        }
        // Pointed through a name: it must lead to lanstash, or have no rewrite yet (Apply adds it).
        let via_missing = match via {
            Some(v) if through.is_none() => {
                if resolve(current, v) != v.trim_end_matches('.') {
                    return Err(format!("{v} does not lead to an address in the rewrites"));
                }
                true
            }
            Some(v) if through != Some(answer.as_str()) => {
                return Err(format!(
                    "{v} leads to {}, not to lanstash's {answer}: change its rewrite, or point the names elsewhere",
                    through.unwrap_or_default()
                ));
            }
            _ => false,
        };
        let write_as = match (via, named, answer_from) {
            (Some(v), _, _) => v.to_owned(),
            (None, Some(name), _) => name.to_owned(),
            (None, None, AnswerFrom::Config) => answer.clone(),
            (None, None, AnswerFrom::Rewrites) => {
                ways.into_iter().max_by_key(|(_, n)| *n).map_or_else(|| answer.clone(), |(a, _)| a.to_owned())
            }
        };
        // A name the answers go through is lanstash's own, not a name lanstash fails to want.
        let aliases: BTreeSet<&str> = ours_list.iter().map(|r| r.answer.as_str()).filter(|a| !is_address(a)).collect();
        let missing = self.wanted.iter().filter(|p| !ours.contains_key(p.as_str())).cloned().collect();
        let duplicates = ours
            .iter()
            .filter(|(p, n)| **n > 1 && self.wanted.iter().any(|w| w == *p))
            .map(|(p, _)| (*p).to_owned())
            .collect();
        let extra = ours
            .keys()
            .filter(|p| !self.wanted.iter().any(|w| w == *p) && !aliases.iter().any(|a| a.eq_ignore_ascii_case(p)))
            .map(|p| (*p).to_owned())
            .collect();
        // A pattern's service: whose host rules take the name (`*.x`: a name under x).
        let services = self
            .wanted
            .iter()
            .map(|p| (p.clone(), self.hosts.classify(&covered::sample(p)).unwrap_or("other").to_owned()))
            .collect();
        let via = via.map(|v| v.trim_end_matches('.').to_owned());
        Ok(Plan {
            answer,
            answer_from,
            write_as,
            via,
            via_missing,
            wanted: self.wanted.len(),
            services,
            missing,
            covered: Vec::new(),
            duplicates,
            extra,
        })
    }

    /// The plan against `server`'s rewrites, with what its rules already cover set apart.
    fn plan_of(&self, server: &dyn DnsServer, config: &DnsConfig) -> Result<(Vec<Rewrite>, Plan), String> {
        let current = server.rewrites()?;
        let mut plan = self.plan_for(&current, config.answer.as_deref(), config.via.as_deref())?;
        covered::split(server, &current, &mut plan);
        Ok((current, plan))
    }

    /// The report: the plan against the server's rewrites, its lookups, lanstash's refusals.
    ///
    /// # Errors
    /// No server set up.
    pub fn report(&self) -> Result<Report, String> {
        let (server, config) = self.server()?;
        let plan = self.plan_of(server.as_ref(), &config).map(|(_, p)| p);
        let (missed, lookups, oldest_ms, lookups_error) = match server.lookups(LOOKUPS) {
            Ok(list) => {
                let mut by_name: BTreeMap<String, Missed> = BTreeMap::new();
                for l in list.iter().filter(|l| !l.rewritten) {
                    let Some(service) = self.hosts.classify(&l.name) else { continue };
                    let m = by_name.entry(l.name.to_ascii_lowercase()).or_insert_with(|| Missed {
                        name: l.name.to_ascii_lowercase(),
                        service: service.to_owned(),
                        count: 0,
                        clients: Vec::new(),
                    });
                    m.count += 1;
                    if m.clients.len() < 5 && !m.clients.contains(&l.client) {
                        m.clients.push(l.client.clone());
                    }
                }
                let mut missed: Vec<Missed> = by_name.into_values().collect();
                missed.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
                let oldest = list.iter().map(|l| l.at_ms).filter(|t| *t > 0).min();
                (missed, list.len(), oldest, None)
            }
            Err(e) => (Vec::new(), 0, None, Some(e)),
        };
        let (refused, refused_others) = self.cache.refused.list();
        Ok(Report {
            kind: server.kind().to_owned(),
            plan,
            missed,
            lookups,
            oldest_ms,
            lookups_error,
            refused,
            refused_others,
        })
    }

    /// Adds the missing rewrites and leaves one of each duplicate (the server removes every
    /// copy of a rewrite at once, so it is removed and added once more).
    ///
    /// # Errors
    /// The server could not be asked, or no address; what was done before a failure stays done.
    pub fn apply(&self) -> Result<Applied, String> {
        let (server, config) = self.server()?;
        let (current, plan) = self.plan_of(server.as_ref(), &config)?;
        // The name the names point at, first, so it leads to lanstash before they do.
        if plan.via_missing
            && let Some(via) = &plan.via
        {
            server.add(&Rewrite { pattern: via.clone(), answer: plan.answer.clone() })?;
        }
        let rewrite = |p: &String| Rewrite { pattern: p.clone(), answer: plan.write_as.clone() };
        for p in &plan.duplicates {
            // Every copy of it that leads to lanstash goes (whichever way it answers), then one comes back.
            let copies: BTreeSet<&Rewrite> =
                current.iter().filter(|r| &r.pattern == p && resolve(&current, &r.answer) == plan.answer).collect();
            for r in copies {
                server.remove(r)?;
            }
            server.add(&rewrite(p))?;
        }
        for p in &plan.missing {
            server.add(&rewrite(p))?;
        }
        Ok(Applied { added: plan.missing.len(), deduplicated: plan.duplicates.len() })
    }
}
