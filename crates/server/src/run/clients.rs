//! Who downloads what: the bytes sent per client and game (counted by the cache),
//! and what the clients are called (names given in the web UI, kept in the
//! database; else what the LAN's DNS servers call their addresses).

use std::sync::Arc;
use std::time::Duration;

use lanstash::config::{ClientNamesFrom, Config};
use lanstash_adapter_dns::reverse::{PtrResolver, nameservers};
use lanstash_adapter_os::pause::Sleep;
use lanstash_adapter_sqlite::db::Database;
use lanstash_app::client_names::{ClientNames, Seen};
use lanstash_app::serve::Cache;
use lanstash_app::serve::clients::ClientTraffic;
use lanstash_ports::clients::ReverseNames;
use lanstash_ports::clock::Clock;

/// A client is downloading a game while it got some of it in the last this long.
const NOW_MS: u64 = 10_000;
/// How long a reverse lookup waits for each server.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Where the machine's DNS servers are listed.
const RESOLV_CONF: &str = "/etc/resolv.conf";

/// The cache's count of bytes per client and game.
pub(super) fn traffic(clock: &Arc<dyn Clock>) -> Arc<ClientTraffic> {
    Arc::new(ClientTraffic::new(Arc::clone(clock), NOW_MS))
}

/// What the clients are called, looked up on a thread of its own for the clients `cache` saw.
pub(super) fn names(
    c: &Config,
    db: &Arc<Database>,
    cache: &Arc<Cache>,
    clock: &Arc<dyn Clock>,
) -> Result<Arc<ClientNames>, String> {
    let servers = match &c.client_names {
        ClientNamesFrom::Off => Vec::new(),
        ClientNamesFrom::Servers(s) => s.clone(),
        ClientNamesFrom::System => std::fs::read_to_string(RESOLV_CONF).map(|t| nameservers(&t)).unwrap_or_default(),
    };
    let reverse =
        (!servers.is_empty()).then(|| Arc::new(PtrResolver::new(servers, LOOKUP_TIMEOUT)) as Arc<dyn ReverseNames>);
    let names = Arc::new(ClientNames::new(Arc::clone(db) as _, reverse, Arc::clone(clock))?);
    let cache = Arc::clone(cache);
    let seen: Seen = Arc::new(move || cache.clients().map(|t| t.clients()).unwrap_or_default());
    names.keep_going(seen, Arc::new(Sleep))?;
    Ok(names)
}
