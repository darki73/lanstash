//! Resolves origin host names by asking an upstream DNS server directly over
//! UDP. The local resolver can't be used: on a LAN cache, local DNS points the
//! CDN names at the cache itself.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lanstash_domain::dns::{DnsError, TYPE_A, build_query, parse_a_response};
use lanstash_ports::clock::Clock;

use crate::udp::exchange;

/// Host name → IPv4 addresses.
pub trait Resolve: Send + Sync {
    /// # Errors
    /// The name could not be resolved.
    fn resolve(&self, host: &str) -> Result<Vec<Ipv4Addr>, String>;
}

/// Resolver settings.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Tried in order.
    pub servers: Vec<SocketAddr>,
    /// Per query and server.
    pub timeout: Duration,
    /// Cached answers live at least this long (CDNs use very short TTLs)…
    pub min_ttl_secs: u32,
    /// …and at most this long.
    pub max_ttl_secs: u32,
    /// Queries per server before the next is tried.
    pub attempts: u32,
    /// When no server answers, an answer expired at most this long ago is used.
    pub stale_secs: u32,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            servers: vec![SocketAddr::from(([1, 1, 1, 1], 53))],
            timeout: Duration::from_secs(2),
            min_ttl_secs: 30,
            max_ttl_secs: 3600,
            attempts: 2,
            stale_secs: 3600,
        }
    }
}

/// UDP DNS resolver with a TTL cache.
///
/// One query at a time: when an answer expires while many downloads start
/// (a prefill, a client opening dozens of connections), the first asks and
/// the rest wait for its answer instead of all asking at once (public
/// resolvers leave bursts of parallel queries unanswered, which stalls
/// downloads for seconds). Each server is asked
/// [`ResolverConfig::attempts`] times; when none answers, an answer expired
/// less than [`ResolverConfig::stale_secs`] ago is used.
pub struct UdpResolver {
    cfg: ResolverConfig,
    clock: Arc<dyn Clock>,
    /// Host → addresses and when they expire (kept past that for stale use).
    cache: Mutex<HashMap<String, (Vec<Ipv4Addr>, u64)>>,
    /// Held while asking the servers.
    asking: Mutex<()>,
    next_id: AtomicU16,
}

impl std::fmt::Debug for UdpResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UdpResolver").field("cfg", &self.cfg).finish_non_exhaustive()
    }
}

impl UdpResolver {
    #[must_use]
    pub fn new(cfg: ResolverConfig, clock: Arc<dyn Clock>) -> Self {
        // Query ids only need to differ between concurrent queries; the
        // response must also come from the server we asked.
        let seed = (clock.now_ms() as u16) ^ 0x5A5A;
        Self { cfg, clock, cache: Mutex::new(HashMap::new()), asking: Mutex::new(()), next_id: AtomicU16::new(seed) }
    }

    fn query(&self, server: SocketAddr, host: &str) -> Result<(Vec<Ipv4Addr>, u32), String> {
        let id = self.next_id.fetch_add(0x9E37, Ordering::Relaxed);
        let q = build_query(id, host, TYPE_A).map_err(|e| e.to_string())?;
        exchange(server, &q, self.cfg.timeout, |b| match parse_a_response(b, id, host) {
            Ok(a) if a.is_empty() => Some(Err(format!("{host}: no A records"))),
            Ok(a) => {
                let ttl = a.iter().map(|r| r.ttl_secs).min().unwrap_or(0);
                Some(Ok((a.iter().map(|r| r.addr).collect(), ttl)))
            }
            Err(DnsError::NotOurResponse) => None,
            Err(e) => Some(Err(format!("{host}: {e}"))),
        })
    }
}

impl Resolve for UdpResolver {
    fn resolve(&self, host: &str) -> Result<Vec<Ipv4Addr>, String> {
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            return Ok(vec![ip]);
        }
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let fresh = |now: u64| {
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&host)
                .filter(|(_, expires)| *expires > now)
                .map(|(a, _)| a.clone())
        };
        if let Some(a) = fresh(self.clock.now_ms()) {
            return Ok(a);
        }
        let _asking = self.asking.lock().unwrap_or_else(PoisonError::into_inner);
        // Someone else may have asked while this waited.
        let now = self.clock.now_ms();
        if let Some(a) = fresh(now) {
            return Ok(a);
        }
        let mut last = String::from("no DNS servers configured");
        for &server in &self.cfg.servers {
            for _ in 0..self.cfg.attempts.max(1) {
                match self.query(server, &host) {
                    Ok((addrs, ttl)) => {
                        let ttl = ttl.clamp(self.cfg.min_ttl_secs, self.cfg.max_ttl_secs);
                        let expires = now + u64::from(ttl) * 1000;
                        self.cache
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .insert(host, (addrs.clone(), expires));
                        return Ok(addrs);
                    }
                    Err(e) => last = e,
                }
            }
        }
        let stale = u64::from(self.cfg.stale_secs) * 1000;
        if let Some((addrs, _)) = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&host)
            .filter(|(_, expires)| expires.saturating_add(stale) > now)
        {
            return Ok(addrs.clone());
        }
        Err(last)
    }
}
