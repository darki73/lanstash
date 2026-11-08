//! What is kept beyond a restart, in one SQLite database in the state
//! directory (`lanstash.db`): the build history, the counters by the minute
//! (every counter group of the status, the HTTPS pass-through's, and each game's
//! bytes sent to clients), and the prefill jobs that ended.

use std::sync::Arc;

use lanstash::config::Config;
use lanstash_adapter_admin_api::api::Counters;
use lanstash_adapter_http::tls_passthrough::TlsCounters;
use lanstash_adapter_os::pause::Sleep;
use lanstash_adapter_sqlite::db::Database;
use lanstash_app::counters_kept::{CounterSource, CountersKept, KeptOptions};
use lanstash_app::prefill_runs::PrefillRuns;
use lanstash_app::status::Status;
use lanstash_ports::clock::Clock;
use lanstash_ports::platform::Platform;

/// The database's file in the state directory.
pub(super) const FILE: &str = "lanstash.db";

/// Opens (or makes) the database.
pub(super) fn database(c: &Config) -> Result<Arc<Database>, String> {
    Database::open(&c.state.join(FILE)).map(Arc::new)
}

/// The counter groups beyond the status's: the HTTPS pass-through's, where it runs.
pub(super) fn https(tls: Option<&Arc<TlsCounters>>) -> Vec<(&'static str, Counters)> {
    tls.map(|t| {
        let t = Arc::clone(t);
        ("https", Arc::new(move || t.values()) as Counters)
    })
    .into_iter()
    .collect()
}

/// Keeps the counters by the minute, on a thread of its own.
pub(super) fn counters(
    db: &Arc<Database>,
    status: &Arc<Status>,
    more: Vec<(&'static str, Counters)>,
    clock: &Arc<dyn Clock>,
) -> Result<Arc<CountersKept>, String> {
    let status = Arc::clone(status);
    let source: CounterSource = Arc::new(move || {
        let mut all: Vec<(String, u64)> = Vec::new();
        // The counters alone: a whole snapshot walks the index for the per-game usage.
        for g in status.counters() {
            all.extend(g.values.iter().map(|(n, v)| (format!("{}.{n}", g.name), *v)));
        }
        for (group, values) in &more {
            all.extend(values().iter().map(|(n, v)| (format!("{group}.{n}"), *v)));
        }
        // Each game's bytes sent to clients (`served/<service>/<game>`), and per client
        // (`client/<address>/<service>/<game>/<hit|miss>`).
        all.extend(status.cache().served.counters());
        all.extend(status.cache().clients().map(|t| t.counters()).unwrap_or_default());
        all
    });
    let kept = Arc::new(CountersKept::new(source, Arc::clone(db) as _, Arc::clone(clock), KeptOptions::default()));
    kept.keep_going(Arc::new(Sleep))?;
    Ok(kept)
}

/// Keeps every platform's prefill jobs as they end, on a thread of its own.
pub(super) fn prefill_runs(db: &Arc<Database>, platforms: &[Arc<dyn Platform>]) -> Result<Arc<PrefillRuns>, String> {
    let jobs = platforms.iter().filter_map(|p| Some((p.id().to_owned(), p.prefill()?))).collect();
    let runs = Arc::new(PrefillRuns::new(jobs, Arc::clone(db) as _));
    runs.keep_going(Arc::new(Sleep))?;
    Ok(runs)
}
