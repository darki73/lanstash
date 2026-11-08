//! Setting up the DNS server from the web UI. A setup is checked first: a kind
//! lanstash knows, a host (a name or an address, no scheme or path), a port, a
//! user, lanstash's address when one is given (an IPv4 or IPv6 address), and
//! over HTTPS how its certificate is trusted (the public roots, one pinned
//! certificate's SHA-256, or not checked).
//! Then it is tried: the server's rewrites are listed with it. Only a setup
//! that works is kept; a test tries one without keeping it. A password left
//! out keeps the one kept before.

use lanstash_ports::dns::{DnsConfig, DnsKind};

use super::DnsLink;

/// What trying a setup found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tried {
    /// The server, in words.
    pub kind: String,
    /// How many rewrites it has.
    pub rewrites: usize,
}

fn host_ok(h: &str) -> bool {
    !h.is_empty() && h.len() <= 253 && h.bytes().all(|b| b.is_ascii_alphanumeric() || b"-.:[]".contains(&b))
}

/// Whether `a` reads as an IPv4 address (four numbers up to 255) or an IPv6 one (hex groups and colons).
fn address_ok(a: &str) -> bool {
    let v4 = a.split('.').count() == 4
        && a.split('.').all(|p| !p.is_empty() && p.len() <= 3 && p.parse::<u16>().is_ok_and(|n| n <= 255));
    let v6 = a.contains(':') && a.len() <= 45 && a.bytes().all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.');
    v4 || v6
}

/// `config` tidied (trimmed, the host in lower case, an empty address none), or what is wrong with it.
///
/// # Errors
/// An unknown kind, or a host, port, user or address that cannot be one.
pub fn check(config: &DnsConfig, kinds: &[DnsKind]) -> Result<DnsConfig, String> {
    let kind = config.kind.trim();
    let Some(of) = kinds.iter().find(|k| k.id == kind) else {
        let known: Vec<&str> = kinds.iter().map(|k| k.id.as_str()).collect();
        return Err(format!("unknown DNS server {kind:?} (known: {})", known.join(", ")));
    };
    let host = config.host.trim().to_ascii_lowercase();
    if !host_ok(&host) {
        return Err("the server's address: a host name or an IP address, without http:// or a path".into());
    }
    // The DNS server is another machine: what listens on lanstash's own loopback is not asked.
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let loopback = bare == "localhost"
        || bare.ends_with(".localhost")
        || bare.parse::<core::net::IpAddr>().is_ok_and(|ip| ip.to_canonical().is_loopback() || ip.is_unspecified());
    if loopback {
        return Err("the server's address: another machine's, not this one's loopback".into());
    }
    if config.port == 0 {
        return Err("the server's port: 1 to 65535".into());
    }
    // A kind that signs in with a password alone keeps no user name.
    let user = if of.user { config.user.trim() } else { "" };
    if of.user && user.is_empty() {
        return Err("a user name is needed".into());
    }
    // Kept one setting per line, and sent before a `:` in a sign-in.
    if user.chars().any(|c| c.is_control() || c == ':') {
        return Err("the user name: no line breaks or other control characters, no ':'".into());
    }
    let answer = config.answer.as_deref().map(str::trim).filter(|a| !a.is_empty());
    // What the rewrites answer: lanstash's address, or a name that leads to it.
    if let Some(a) = answer
        && !address_ok(a)
        && !(host_ok(a) && a.contains('.') && !a.contains(':') && a.bytes().any(|b| b.is_ascii_alphabetic()))
    {
        return Err(format!("what the rewrites answer: {a:?} is neither an IP address nor a host name"));
    }
    let name_ok =
        |a: &str| host_ok(a) && a.contains('.') && !a.contains(':') && a.bytes().any(|b| b.is_ascii_alphabetic());
    let via =
        config.via.as_deref().map(|v| v.trim().trim_end_matches('.').to_ascii_lowercase()).filter(|v| !v.is_empty());
    if let Some(v) = &via
        && !name_ok(v)
    {
        return Err(format!("the name to point the names at: {v:?} is not a host name"));
    }
    // A pin, or no checking, only means something over HTTPS.
    let pin = match config.pin.as_deref().map(pin).filter(|p| config.https && !p.is_empty()) {
        Some(p) if p.len() != 95 => return Err("a pinned certificate is its SHA-256: 32 hex pairs".into()),
        p => p,
    };
    Ok(DnsConfig {
        kind: kind.to_owned(),
        host,
        port: config.port,
        https: config.https,
        pin,
        unchecked: config.unchecked && config.https,
        user: user.to_owned(),
        answer: answer.map(str::to_owned),
        via,
    })
}

/// A certificate's SHA-256 as written by a person (any case, colons, spaces or none), as
/// upper-case hex pairs split by colons; what is not hex is dropped.
#[must_use]
pub fn pin(text: &str) -> String {
    let hex: Vec<char> = text.chars().filter(char::is_ascii_hexdigit).map(|c| c.to_ascii_uppercase()).collect();
    hex.chunks(2).map(|p| p.iter().collect::<String>()).collect::<Vec<_>>().join(":")
}

impl DnsLink {
    /// Checks and tries `config` (with `password`, else the kept one) and, unless only
    /// testing, keeps it. The kept password goes only to the server it was kept for (same
    /// kind, host, port, user and way of checking its certificate): for another, it is asked
    /// for again.
    ///
    /// # Errors
    /// What is wrong with it, why the server could not be asked, or why it was not kept.
    pub fn configure(&self, config: &DnsConfig, password: Option<&str>, keep: bool) -> Result<Tried, String> {
        let setup = self.setup();
        let config = check(config, &setup.kinds())?;
        let same_server = |k: &DnsConfig| {
            (&k.kind, &k.host, k.port, k.https, &k.pin, k.unchecked, &k.user)
                == (&config.kind, &config.host, config.port, config.https, &config.pin, config.unchecked, &config.user)
        };
        let password = match (password.filter(|p| !p.is_empty()), setup.config()) {
            (Some(p), _) => p.to_owned(),
            (None, Some(k)) if same_server(&k) => setup.password().ok_or("a password is needed")?,
            (None, Some(_)) => return Err("a password is needed for another server".into()),
            (None, None) => return Err("a password is needed".into()),
        };
        let server = setup.connect(&config, &password).ok_or_else(|| format!("unknown DNS server {}", config.kind))?;
        let rewrites = server.rewrites()?.len();
        if keep {
            setup.save(&config, &password)?;
        }
        Ok(Tried { kind: server.kind().to_owned(), rewrites })
    }

    /// Forgets the setup and its password.
    ///
    /// # Errors
    /// They could not be removed.
    pub fn forget(&self) -> Result<(), String> {
        self.setup().forget()
    }
}
