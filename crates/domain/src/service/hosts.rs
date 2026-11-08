//! Host name → service, from cache-domains lists (one `<service>.txt` per
//! service).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    /// Lower-case host name without a leading `*.`.
    name: String,
    /// `*.name`: subdomains only.
    wildcard: bool,
    service: String,
}

/// Host name → service identifier (`steam`, `blizzard`, `epicgames`, `riot`, …).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostRules {
    rules: Vec<Rule>,
}

impl HostRules {
    /// Adds one host pattern (`name` matches itself and its subdomains,
    /// `*.name` only subdomains).
    pub fn add_host(&mut self, service: &str, pattern: &str) {
        let p = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
        let (name, wildcard) = match p.strip_prefix("*.") {
            Some(rest) => (rest.to_string(), true),
            None => (p, false),
        };
        if !name.is_empty() && !name.contains(['/', ' ', '*']) {
            self.rules.push(Rule { name, wildcard, service: service.to_ascii_lowercase() });
        }
    }

    /// Adds a cache-domains list (one pattern per line, `#` comments). Returns
    /// the number of patterns added.
    pub fn add_list(&mut self, service: &str, text: &str) -> usize {
        let before = self.rules.len();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if !line.is_empty() {
                self.add_host(service, line);
            }
        }
        self.rules.len() - before
    }

    /// The service for `host` (trailing dot and case ignored), if listed.
    #[must_use]
    pub fn classify(&self, host: &str) -> Option<&str> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        self.rules
            .iter()
            .find(|r| {
                let sub = host.len() > r.name.len()
                    && host.ends_with(r.name.as_str())
                    && host.as_bytes()[host.len() - r.name.len() - 1] == b'.';
                sub || (!r.wildcard && host == r.name)
            })
            .map(|r| r.service.as_str())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Distinct service names, in the order first added.
    #[must_use]
    pub fn services(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for r in &self.rules {
            if !out.contains(&r.service.as_str()) {
                out.push(&r.service);
            }
        }
        out
    }
}
