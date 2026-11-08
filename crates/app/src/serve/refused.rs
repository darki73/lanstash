//! The hosts the cache refused (403, a host no cache-domains list names), and
//! how often since the start: a game CDN missing from the lists shows up here.
//! It keeps at most [`MAX_HOSTS`] names;
//! later new ones are only counted as others.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// The most names kept.
pub const MAX_HOSTS: usize = 256;

#[derive(Debug, Default)]
struct Tally {
    hosts: BTreeMap<String, u64>,
    others: u64,
}

/// Refused hosts and their counts.
#[derive(Debug, Default)]
pub struct RefusedHosts(Mutex<Tally>);

impl RefusedHosts {
    /// Counts a refusal of `host`.
    pub fn note(&self, host: &str) {
        let mut t = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(n) = t.hosts.get_mut(host) {
            *n += 1;
        } else if t.hosts.len() < MAX_HOSTS {
            t.hosts.insert(host.to_owned(), 1);
        } else {
            t.others += 1;
        }
    }

    /// The refused hosts, most refused first, and the refusals of hosts beyond the kept ones.
    #[must_use]
    pub fn list(&self) -> (Vec<(String, u64)>, u64) {
        let t = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut v: Vec<(String, u64)> = t.hosts.iter().map(|(h, n)| (h.clone(), *n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        (v, t.others)
    }
}
