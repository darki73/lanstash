//! The DNS server of the network (AdGuard Home, Pi-hole, ...), as lanstash
//! works with it: the rewrites that send game CDN names to the cache, and the
//! names clients looked up lately ([`DnsServer`]). A rewrite's pattern is
//! written as in the cache-domains lists: `name` is that one name, `*.name`
//! its subdomains (not the name itself); each server's adapter turns it into
//! its own rules. Which server it is, where, and how lanstash signs in, is set
//! up from the web UI ([`DnsSetup`]).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// A kind of DNS server lanstash can work with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsKind {
    /// Its id in the setup (`adguard`).
    pub id: String,
    /// Its name in words (`AdGuard Home`).
    pub name: String,
    /// The port its admin API usually listens on, over HTTP and over HTTPS.
    pub port: u16,
    pub https_port: u16,
    /// It signs in with a user name and a password (else a password alone).
    pub user: bool,
}

/// Where the network's DNS server is and who lanstash signs in as (the password is kept apart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsConfig {
    /// A [`DnsKind`] id.
    pub kind: String,
    pub host: String,
    pub port: u16,
    /// Its admin API is HTTPS.
    pub https: bool,
    /// The SHA-256 of its certificate, when that one certificate is trusted (self-signed or privately issued).
    pub pin: Option<String>,
    /// Its certificate is not checked at all.
    pub unchecked: bool,
    pub user: String,
    /// The address the rewrites answer for lanstash; none: the one its names' rewrites already answer.
    pub answer: Option<String>,
    /// A name the game names are pointed at, which is pointed at lanstash's address (one rewrite to
    /// change if the address changes); none: each name answers the address itself.
    pub via: Option<String>,
}

/// The DNS server's setup, kept between runs.
pub trait DnsSetup: Send + Sync {
    /// The kinds of server it can connect to.
    fn kinds(&self) -> Vec<DnsKind>;

    /// The setup, if one was saved.
    fn config(&self) -> Option<DnsConfig>;

    /// The password kept for it, if any.
    fn password(&self) -> Option<String>;

    /// Keeps `config` and `password`.
    ///
    /// # Errors
    /// They could not be written.
    fn save(&self, config: &DnsConfig, password: &str) -> Result<(), String>;

    /// Forgets the setup and its password.
    ///
    /// # Errors
    /// They could not be removed.
    fn forget(&self) -> Result<(), String>;

    /// A server of `config`'s kind (none for a kind it does not know).
    fn connect(&self, config: &DnsConfig, password: &str) -> Option<Arc<dyn DnsServer>>;
}

/// A name (or names) the server answers with an address of its own choosing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rewrite {
    /// `name` or `*.name`.
    pub pattern: String,
    /// The address answered.
    pub answer: String,
}

/// A name a client looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    pub name: String,
    /// The client's address.
    pub client: String,
    /// The addresses answered (none for a failed lookup).
    pub answers: Vec<String>,
    /// Whether a rewrite answered it.
    pub rewritten: bool,
    /// When (ms since the epoch).
    pub at_ms: u64,
}

/// How the server answers a name now, everything it answers from counted (its
/// rewrites, its own rules, the filter lists it follows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// The address, or the name, it answers with itself; none: it asks upstream, or blocks the name.
    pub to: Option<String>,
    /// The rule that answers, as written (none when a rewrite answers, or nothing does).
    pub rule: Option<String>,
}

/// The network's DNS server.
pub trait DnsServer: Send + Sync {
    /// What it is, in words ("AdGuard Home").
    fn kind(&self) -> &str;

    /// Its rewrites, all of them (not only lanstash's).
    ///
    /// # Errors
    /// The server could not be asked, or refused.
    fn rewrites(&self) -> Result<Vec<Rewrite>, String>;

    /// Adds a rewrite (the caller makes sure it is not there yet).
    ///
    /// # Errors
    /// The server could not be asked, or refused.
    fn add(&self, rewrite: &Rewrite) -> Result<(), String>;

    /// Removes a rewrite.
    ///
    /// # Errors
    /// The server could not be asked, or refused.
    fn remove(&self, rewrite: &Rewrite) -> Result<(), String>;

    /// The latest lookups, newest first, at most `limit`.
    ///
    /// # Errors
    /// The server could not be asked, or refused (or keeps no log).
    fn lookups(&self, limit: usize) -> Result<Vec<Lookup>, String>;

    /// How it answers `name` now (none: it cannot say, only its rewrites count).
    ///
    /// # Errors
    /// The server could not be asked, or refused.
    fn check(&self, name: &str) -> Option<Result<Answered, String>> {
        let _ = name;
        None
    }
}
