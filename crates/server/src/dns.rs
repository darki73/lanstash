//! The network's DNS server as the web UI sets it up ([`LiveDns`]): which kind
//! (AdGuard Home, Pi-hole; others join as their crates do), where, and who
//! lanstash signs in as, kept in the state directory (`dns`, `key = value`
//! lines); the password apart, among the secrets (`dns-password`). And the
//! patterns lanstash wants rewritten ([`patterns`]).

use std::sync::Arc;

use lanstash_dns_adguard::server::AdGuard;
use lanstash_dns_pihole::server::PiHole;
use lanstash_ports::dns::{DnsConfig, DnsKind, DnsServer, DnsSetup};
use lanstash_ports::keep::Keep;
use lanstash_ports::secrets::Secrets;
use lanstash_ports::web::Endpoint;
use lanstash_ports::web::Web;

/// The name the setup is kept under.
pub const KEPT: &str = "dns";
/// The name the password is kept under.
pub const PASSWORD: &str = "dns-password";

/// The DNS patterns of the cached services: each list's lines as written (`name`,
/// `*.name`; comments and blanks left out), then for each built-in `host` (which means
/// the host and its subdomains) both `host` and `*.host`. Each once, in that order.
#[must_use]
pub fn patterns(lists: &[&str], builtin: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let lines = lists.iter().flat_map(|t| t.lines()).map(|l| l.split('#').next().unwrap_or("").trim());
    let builtin = builtin.iter().flat_map(|h| [(*h).to_owned(), format!("*.{h}")]);
    for p in lines.filter(|l| !l.is_empty()).map(|l| l.to_ascii_lowercase()).chain(builtin) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The setup, kept between runs.
pub struct LiveDns {
    keep: Arc<dyn Keep>,
    secrets: Arc<dyn Secrets>,
    web: Arc<dyn Web>,
}

impl std::fmt::Debug for LiveDns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveDns").finish_non_exhaustive()
    }
}

impl LiveDns {
    #[must_use]
    pub fn new(keep: Arc<dyn Keep>, secrets: Arc<dyn Secrets>, web: Arc<dyn Web>) -> Self {
        Self { keep, secrets, web }
    }
}

/// A setup from its kept lines; none without a kind and a host.
#[must_use]
pub fn read(text: &str) -> Option<DnsConfig> {
    let mut c = DnsConfig {
        kind: String::new(),
        host: String::new(),
        port: 0,
        https: false,
        pin: None,
        unchecked: false,
        user: String::new(),
        answer: None,
        via: None,
    };
    for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
        let v = v.trim().to_owned();
        match k.trim() {
            "kind" => c.kind = v,
            "host" => c.host = v,
            "port" => c.port = v.parse().unwrap_or(0),
            "https" => c.https = v == "true",
            "pin" if !v.is_empty() => c.pin = Some(v),
            "unchecked" => c.unchecked = v == "true",
            "user" => c.user = v,
            "answer" if !v.is_empty() => c.answer = Some(v),
            "via" if !v.is_empty() => c.via = Some(v),
            _ => {}
        }
    }
    (!c.kind.is_empty() && !c.host.is_empty()).then_some(c)
}

/// A setup as kept lines.
#[must_use]
pub fn write(c: &DnsConfig) -> String {
    format!(
        "kind = {}\nhost = {}\nport = {}\nhttps = {}\npin = {}\nunchecked = {}\nuser = {}\nanswer = {}\nvia = {}\n",
        c.kind,
        c.host,
        c.port,
        c.https,
        c.pin.as_deref().unwrap_or(""),
        c.unchecked,
        c.user,
        c.answer.as_deref().unwrap_or(""),
        c.via.as_deref().unwrap_or("")
    )
}

impl DnsSetup for LiveDns {
    fn kinds(&self) -> Vec<DnsKind> {
        vec![
            DnsKind { id: "adguard".into(), name: "AdGuard Home".into(), port: 3000, https_port: 443, user: true },
            DnsKind { id: "pihole".into(), name: "Pi-hole".into(), port: 80, https_port: 443, user: false },
        ]
    }

    fn config(&self) -> Option<DnsConfig> {
        let b = self.keep.load(KEPT).ok()??;
        read(&String::from_utf8_lossy(&b))
    }

    /// The password among the secrets; one found beside the setup instead is moved there.
    fn password(&self) -> Option<String> {
        if let Some(p) = self.secrets.load(PASSWORD).ok().flatten() {
            return Some(p);
        }
        let p = String::from_utf8(self.keep.load(PASSWORD).ok()??).ok()?;
        if self.secrets.save(PASSWORD, &p).is_ok() {
            let _ = self.keep.remove(PASSWORD);
        }
        Some(p)
    }

    fn save(&self, config: &DnsConfig, password: &str) -> Result<(), String> {
        self.secrets.save(PASSWORD, password)?;
        self.keep.save(KEPT, write(config).as_bytes())
    }

    fn forget(&self) -> Result<(), String> {
        self.secrets.remove(PASSWORD)?;
        self.keep.save(KEPT, b"")
    }

    fn connect(&self, config: &DnsConfig, password: &str) -> Option<Arc<dyn DnsServer>> {
        let at = Endpoint {
            host: config.host.clone(),
            port: config.port,
            https: config.https,
            pin: config.pin.clone(),
            unchecked: config.unchecked,
        };
        match config.kind.as_str() {
            "adguard" => Some(Arc::new(AdGuard::new(Arc::clone(&self.web), at, &config.user, password))),
            "pihole" => Some(Arc::new(PiHole::new(Arc::clone(&self.web), at, password))),
            _ => None,
        }
    }
}
