//! Configuration: `key = value` lines from a file, then
//! the same keys as command-line flags (`--fetch-ahead 8`), later wins. Pure:
//! text in, [`Config`] out; the caller reads the file.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use lanstash_domain::evict::KeepFree;

pub mod values;
use values::{boolean, client_names, dns_server, every_days, keep_free, list, paths, size};

/// Every key, its default (`None` = no default) and whether it is a switch (a flag without a value).
const KEYS: &[(&str, Option<&str>, bool)] = &[
    ("listen", Some("0.0.0.0:80"), false),
    ("listen_tls", None, false),
    ("admin_listen", None, false),
    ("admin_listen_tls", None, false),
    ("admin_tls_names", None, false),
    ("web_ui", Some("/usr/local/share/lanstash/web"), false),
    ("store", None, false),
    ("upstream_dns", None, false),
    ("client_names", Some("system"), false),
    ("cache_domains", None, false),
    ("services", Some("steam,blizzard,epicgames,riot"), false),
    ("names", None, false),
    ("blizzard_region", Some("us"), false),
    ("blizzard_tags", Some("Windows,x86_64,enUS"), false),
    ("steam_os", Some("windows"), false),
    ("steam_language", Some("english"), false),
    ("steam_arch", Some("64"), false),
    ("prefill_parallel", Some("8"), false),
    ("auto_update", None, false),
    ("nginx_cache", None, false),
    ("secrets", Some("/var/lib/lanstash/secrets"), false),
    ("state", Some("/var/lib/lanstash/state"), false),
    ("access_log", None, false),
    ("access_log_headers", Some("false"), true),
    ("event_log", None, false),
    ("segment_size", Some("1G"), false),
    ("object_file_min", Some("16M"), false),
    ("fetch_piece", Some("16M"), false),
    ("fetch_ahead", Some("4"), false),
    ("download_window", Some("4M"), false),
    ("send_piece", Some("1M"), false),
    ("prefetch", Some("false"), true),
    ("retries", Some("3"), false),
    ("max_connections", Some("4096"), false),
    ("stats_secs", Some("0"), false),
    ("max_size", None, false),
    ("keep_free", Some("5%"), false),
    ("evict_secs", Some("30"), false),
    ("verify_every", Some("off"), false),
    ("verify_rate", Some("50M"), false),
    ("log_max_size", Some("100M"), false),
    ("log_keep", Some("5"), false),
];

/// `store.<service>` roots are the only keys with a variable part.
const STORE_PREFIX: &str = "store.";
/// The most pieces fetched ahead of a client (each one a download thread a client can start).
pub const MAX_AHEAD: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub listen: SocketAddr,
    /// HTTPS pass-through; `None` = off.
    pub listen_tls: Option<SocketAddr>,
    /// Status page; `None` = off.
    pub admin_listen: Option<SocketAddr>,
    /// The admin API and web UI over HTTPS; with it, `admin_listen` sends other machines here.
    pub admin_listen_tls: Option<SocketAddr>,
    /// The names and addresses the HTTPS certificate is for (none: this machine's name and `localhost`).
    pub admin_tls_names: Vec<String>,
    /// The built web UI served on `admin_listen` (its `index.html` and `assets/`); none there: the built-in page.
    pub web_ui: Option<PathBuf>,
    /// Default store root, then extra mounts of the same directory for reads.
    pub store: Vec<PathBuf>,
    /// Roots of single services.
    pub store_per_service: BTreeMap<String, Vec<PathBuf>>,
    /// Empty only for commands that do not fetch.
    pub upstream_dns: Vec<SocketAddr>,
    /// Where the clients' names are looked up (a reverse DNS query).
    pub client_names: ClientNamesFrom,
    pub cache_domains: Option<PathBuf>,
    /// Names file (`service/game = Name` lines).
    pub names: Option<PathBuf>,
    /// The Battle.net region whose builds count (`us`, `eu`, `kr`, `tw`, `cn`).
    pub blizzard_region: String,
    /// Download-manifest tags of a Battle.net install (platform, architecture, locale, content), plus the region's.
    pub blizzard_tags: Vec<String>,
    /// Who a Steam install is for: OS (`windows`, `linux`, `macos`), language (`english`, ...), arch (`64`, `32`).
    pub steam_os: String,
    pub steam_language: String,
    pub steam_arch: String,
    /// Prefill requests in flight at once (1 to 64).
    pub prefill_parallel: usize,
    /// When games once held whole are updated (`always`, or a daily UTC window `HH:MM-HH:MM`); `None` = never.
    pub auto_update: Option<lanstash_domain::window::Window>,
    /// An nginx (lancache) cache directory to probe and import from (`None` = none).
    pub nginx_cache: Option<PathBuf>,
    /// Directory of private files for secrets (the Steam refresh token).
    pub secrets: PathBuf,
    /// Directory of state kept between runs (work not to redo after a restart).
    pub state: PathBuf,
    pub services: Vec<String>,
    pub access_log: Option<PathBuf>,
    pub access_log_headers: bool,
    /// `None` = standard error.
    pub event_log: Option<PathBuf>,
    pub segment_size: u64,
    pub object_file_min: u64,
    pub fetch_piece: u64,
    pub fetch_ahead: usize,
    pub download_window: usize,
    pub send_piece: usize,
    pub prefetch: bool,
    pub retries: usize,
    pub max_connections: usize,
    pub stats_secs: u64,
    /// Eviction: at most this many bytes stored (`None` = no limit).
    pub max_size: Option<u64>,
    /// Eviction: keep this much of each root's file system free (`None` = no limit).
    pub keep_free: Option<KeepFree>,
    /// Seconds between eviction checks.
    pub evict_secs: u64,
    /// Days between background checks of every stored byte (`weekly`, `monthly`, `<n>d`); `None` = off.
    pub verify_every_days: Option<u64>,
    /// Bytes a second the background check reads at most.
    pub verify_rate: u64,
    /// `access_log` and `event_log` rotate before passing this size (0 = never).
    pub log_max_size: u64,
    /// Rotated log files kept.
    pub log_keep: u32,
}

/// `client_names`: where what the LAN calls a client's address is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientNamesFrom {
    /// The machine's own DNS servers (`/etc/resolv.conf`): the ones that know the LAN's names.
    System,
    /// These servers.
    Servers(Vec<SocketAddr>),
    /// Nowhere: clients are their addresses, or the names given them.
    Off,
}

/// Raw `key = value` settings in order of precedence (later wins).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings(Vec<(String, String)>);

fn known(key: &str) -> Option<(Option<&'static str>, bool)> {
    if key.strip_prefix(STORE_PREFIX).is_some_and(valid_service) {
        return Some((None, false));
    }
    KEYS.iter().find(|(k, ..)| *k == key).map(|(_, d, s)| (*d, *s))
}

/// Whether `key` is a switch (no value on the command line).
#[must_use]
pub fn is_switch(key: &str) -> bool {
    known(key).is_some_and(|(_, s)| s)
}

fn valid_service(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

impl Settings {
    /// Adds one setting.
    ///
    /// # Errors
    /// Unknown key.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let key = key.trim().replace('-', "_");
        if known(&key).is_none() {
            return Err(format!("unknown setting `{key}`"));
        }
        self.0.push((key, value.trim().to_owned()));
        Ok(())
    }

    /// Adds the lines of a configuration file.
    ///
    /// # Errors
    /// A malformed line or unknown key, with its line number.
    pub fn add_file(&mut self, name: &str, text: &str) -> Result<(), String> {
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| format!("{name}:{}: expected `key = value`", n + 1))?;
            self.set(k, v).map_err(|e| format!("{name}:{}: {e}", n + 1))?;
        }
        Ok(())
    }

    /// The settings in order (later wins).
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The value of `key`: the last one set, else its default.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&str> {
        self.get(key)
    }

    /// The value of `key` as set here (no default).
    #[must_use]
    pub fn explicit(&self, key: &str) -> Option<&str> {
        self.0.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// The built-in default of `key`.
    #[must_use]
    pub fn default_of(key: &str) -> Option<&'static str> {
        known(key)?.0
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str()).or_else(|| known(key)?.0)
    }

    fn opt_addr(&self, key: &str) -> Result<Option<SocketAddr>, String> {
        match self.get(key).filter(|v| !v.is_empty()) {
            Some(v) => v.parse().map(Some).map_err(|_| format!("`{key}`: invalid value {v:?}")),
            None => Ok(None),
        }
    }

    fn required(&self, key: &str) -> Result<&str, String> {
        self.get(key).filter(|v| !v.is_empty()).ok_or_else(|| format!("`{key}` is required"))
    }

    fn parsed<T>(&self, key: &str, f: impl FnOnce(&str) -> Option<T>) -> Result<T, String> {
        let v = self.required(key)?;
        f(v).ok_or_else(|| format!("`{key}`: invalid value {v:?}"))
    }

    /// Builds the configuration.
    ///
    /// # Errors
    /// A missing required key or an invalid value.
    pub fn build(&self) -> Result<Config, String> {
        let opt_path = |k: &str| self.get(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let mut store_per_service = BTreeMap::new();
        for (k, v) in &self.0 {
            if let Some(service) = k.strip_prefix(STORE_PREFIX) {
                store_per_service.insert(service.to_owned(), paths(v));
            }
        }
        let services: Vec<String> = list(self.required("services")?).map(|s| s.to_ascii_lowercase()).collect();
        if let Some(bad) = services.iter().find(|s| !valid_service(s)) {
            return Err(format!("`services`: invalid name {bad:?}"));
        }
        let c = Config {
            listen: self.parsed("listen", |v| v.parse().ok())?,
            listen_tls: self.opt_addr("listen_tls")?,
            admin_listen: self.opt_addr("admin_listen")?,
            admin_listen_tls: self.opt_addr("admin_listen_tls")?,
            admin_tls_names: self
                .get("admin_tls_names")
                .map(|v| list(v).map(String::from).collect())
                .unwrap_or_default(),
            store: paths(self.required("store")?),
            store_per_service,
            upstream_dns: match self.get("upstream_dns") {
                Some(v) => {
                    list(v).map(dns_server).collect::<Option<_>>().ok_or(format!("`upstream_dns`: invalid {v:?}"))?
                }
                None => Vec::new(),
            },
            client_names: self.parsed("client_names", client_names)?,
            cache_domains: opt_path("cache_domains"),
            names: opt_path("names"),
            nginx_cache: opt_path("nginx_cache"),
            auto_update: match self.get("auto_update").filter(|v| !v.is_empty() && *v != "off") {
                Some(v) => Some(
                    lanstash_domain::window::Window::parse(if v == "always" { "00:00-24:00" } else { v })
                        .ok_or(format!("`auto_update`: invalid value {v:?} (always, HH:MM-HH:MM in UTC, or off)"))?,
                ),
                None => None,
            },
            secrets: PathBuf::from(self.required("secrets")?),
            state: PathBuf::from(self.required("state")?),
            blizzard_region: self.parsed("blizzard_region", |v| {
                (2..=3)
                    .contains(&v.len())
                    .then(|| v.to_ascii_lowercase())
                    .filter(|v| v.bytes().all(|b| b.is_ascii_lowercase()))
            })?,
            steam_os: self.parsed("steam_os", |v| {
                let v = v.to_ascii_lowercase();
                ["windows", "linux", "macos"].contains(&v.as_str()).then_some(v)
            })?,
            steam_language: self.parsed("steam_language", |v| {
                // One language or several, comma-separated: each a Steam language name.
                let langs: Vec<String> = v.split(',').map(|l| l.trim().to_ascii_lowercase()).collect();
                let ok =
                    |l: &String| (1..=32).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
                (langs.len() <= 8 && langs.iter().all(ok)).then(|| langs.join(","))
            })?,
            steam_arch: self.parsed("steam_arch", |v| ["64", "32"].contains(&v).then(|| v.to_owned()))?,
            prefill_parallel: self.parsed("prefill_parallel", |v| v.parse().ok().filter(|n| (1..=64).contains(n)))?,
            blizzard_tags: self.parsed("blizzard_tags", |v| {
                let tags: Vec<String> = list(v).map(str::to_owned).collect();
                let ok = |t: &String| {
                    (1..=32).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                };
                (!tags.is_empty() && tags.iter().all(ok)).then_some(tags)
            })?,
            services,
            access_log: opt_path("access_log"),
            web_ui: opt_path("web_ui"),
            access_log_headers: self.parsed("access_log_headers", boolean)?,
            event_log: opt_path("event_log"),
            segment_size: self.parsed("segment_size", size)?,
            object_file_min: self.parsed("object_file_min", size)?,
            fetch_piece: self.parsed("fetch_piece", size)?,
            fetch_ahead: self.parsed("fetch_ahead", |v| v.parse().ok().filter(|n| (1..=MAX_AHEAD).contains(n)))?,
            download_window: self.parsed("download_window", |v| size(v).and_then(|n| usize::try_from(n).ok()))?,
            send_piece: self.parsed("send_piece", |v| size(v).and_then(|n| usize::try_from(n).ok()))?,
            prefetch: self.parsed("prefetch", boolean)?,
            retries: self.parsed("retries", |v| v.parse().ok())?,
            max_connections: self.parsed("max_connections", |v| v.parse().ok())?,
            stats_secs: self.parsed("stats_secs", |v| v.parse().ok())?,
            max_size: self
                .get("max_size")
                .filter(|v| !v.is_empty() && *v != "0")
                .map(|v| size(v).ok_or(format!("`max_size`: invalid value {v:?}")))
                .transpose()?,
            keep_free: keep_free(self.get("keep_free").unwrap_or(""))?,
            log_max_size: self.parsed("log_max_size", size)?,
            log_keep: self.parsed("log_keep", |v| v.parse().ok())?,
            evict_secs: self.parsed("evict_secs", |v| v.parse().ok().filter(|n: &u64| *n > 0))?,
            verify_every_days: self.parsed("verify_every", every_days)?,
            verify_rate: self.parsed("verify_rate", |v| size(v).filter(|n| *n >= 1 << 20))?,
        };
        if c.fetch_piece < 64 * 1024 || c.download_window < 64 * 1024 || c.send_piece < 4096 {
            return Err("fetch_piece, download_window ≥ 64K, send_piece ≥ 4K".into());
        }
        Ok(c)
    }
}
