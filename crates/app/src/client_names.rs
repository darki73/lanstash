//! Use case: what the clients are called. A name given to a client wins;
//! else what the network calls its address (a reverse DNS lookup, kept as long
//! as the answer says, between [`MIN_KEEP_MS`] and [`MAX_KEEP_MS`]); else none,
//! and the address stands for it. Lookups run on a thread of their own, for the
//! clients the cache has seen, never while a request waits.

use core::net::IpAddr;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clients::{ClientLabels, ReverseNames};
use lanstash_ports::clock::{Clock, Pause};

/// An answer is kept at least this long…
pub const MIN_KEEP_MS: u64 = 5 * 60_000;
/// …and at most this long.
pub const MAX_KEEP_MS: u64 = 24 * 60 * 60_000;
/// A lookup that failed is tried again after this long.
const RETRY_MS: u64 = 60_000;
/// How often the thread looks for clients to look up.
const LOOK_MS: u64 = 5_000;
/// The longest name a client is given.
pub const MAX_LABEL: usize = 64;

/// What a client is called.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientName {
    /// The name given to it ([`ClientLabels`]).
    pub label: Option<String>,
    /// What the network calls its address.
    pub host: Option<String>,
}

/// The clients seen, by address (what the thread looks up).
pub type Seen = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

/// A lookup's answer: the host name, and until when it is kept.
#[derive(Debug, Clone)]
struct Looked {
    host: Option<String>,
    until_ms: u64,
}

/// What the clients are called.
pub struct ClientNames {
    labels: Arc<dyn ClientLabels>,
    reverse: Option<Arc<dyn ReverseNames>>,
    clock: Arc<dyn Clock>,
    given: Mutex<BTreeMap<String, String>>,
    looked: Mutex<BTreeMap<String, Looked>>,
}

impl std::fmt::Debug for ClientNames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientNames").field("reverse", &self.reverse.is_some()).finish_non_exhaustive()
    }
}

/// `label` as a client's name: trimmed, without control characters, at most [`MAX_LABEL`]
/// characters; empty is none.
///
/// # Errors
/// It is too long.
pub fn clean_label(label: &str) -> Result<Option<String>, String> {
    let l: String = label.trim().chars().filter(|c| !c.is_control()).collect();
    if l.chars().count() > MAX_LABEL {
        return Err(format!("a name has at most {MAX_LABEL} characters"));
    }
    Ok((!l.is_empty()).then_some(l))
}

impl ClientNames {
    /// The names `labels` keeps; addresses looked up through `reverse` (none: never).
    ///
    /// # Errors
    /// The kept names could not be read.
    pub fn new(
        labels: Arc<dyn ClientLabels>,
        reverse: Option<Arc<dyn ReverseNames>>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let given = labels.labels()?.into_iter().collect();
        Ok(Self { labels, reverse, clock, given: Mutex::new(given), looked: Mutex::new(BTreeMap::new()) })
    }

    /// What the client at `address` is called, as far as known now (nothing is looked up).
    #[must_use]
    pub fn name(&self, address: &str) -> ClientName {
        ClientName {
            label: self.given.lock().unwrap_or_else(PoisonError::into_inner).get(address).cloned(),
            host: self.looked.lock().unwrap_or_else(PoisonError::into_inner).get(address).and_then(|l| l.host.clone()),
        }
    }

    /// Names the client at `address` `label` (empty: takes its name away).
    ///
    /// # Errors
    /// The address or the name is not one, or it could not be kept.
    pub fn set_label(&self, address: &str, label: &str) -> Result<(), String> {
        let ip: IpAddr = address.parse().map_err(|_| format!("{address:?} is not an address"))?;
        let address = ip.to_canonical().to_string();
        let label = clean_label(label)?;
        self.labels.set_label(&address, label.as_deref())?;
        let mut given = self.given.lock().unwrap_or_else(PoisonError::into_inner);
        match label {
            Some(l) => given.insert(address, l),
            None => given.remove(&address),
        };
        Ok(())
    }

    /// Looks up those of `addresses` not looked up yet, or whose answer is too old.
    pub fn look_up(&self, addresses: &[String]) {
        let Some(reverse) = &self.reverse else { return };
        for address in addresses {
            let now = self.clock.now_ms();
            let due = self
                .looked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(address)
                .is_none_or(|l| now >= l.until_ms);
            let Ok(ip) = address.parse::<IpAddr>() else { continue };
            if !due {
                continue;
            }
            let looked = match reverse.names_of(ip) {
                Ok((names, ttl)) => Looked {
                    host: names.into_iter().next().map(|n| n.trim_end_matches('.').to_owned()),
                    until_ms: now + (u64::from(ttl) * 1_000).clamp(MIN_KEEP_MS, MAX_KEEP_MS),
                },
                Err(_) => {
                    // Keep what was known; ask again a little later.
                    let before = self.looked.lock().unwrap_or_else(PoisonError::into_inner).get(address).cloned();
                    Looked { host: before.and_then(|l| l.host), until_ms: now + RETRY_MS }
                }
            };
            self.looked.lock().unwrap_or_else(PoisonError::into_inner).insert(address.clone(), looked);
        }
    }

    /// Looks up the clients `seen` returns every few seconds, on a thread of its own (none
    /// without a way to look them up).
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_going(self: &Arc<Self>, seen: Seen, pause: Arc<dyn Pause>) -> Result<(), String> {
        if self.reverse.is_none() {
            return Ok(());
        }
        let me = Arc::clone(self);
        crate::rounds::forever("client-names", Arc::clone(&pause), move || {
            me.look_up(&seen());
            pause.pause_ms(LOOK_MS);
            crate::rounds::Next::Again
        })
    }
}
