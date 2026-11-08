//! The service's health, as the web UI's health page shows it
//! (`GET /api/v1/health`): the checks the service can make of itself, each time
//! it is asked: what the store holds and what loading it at the start could not
//! take, the files the service has open against its limit, the store disk's free
//! space, and what the background check found. (The service's state and its
//! ports are `lanstash doctor`'s to say, from outside.)

use std::sync::Arc;

use lanstash::config::Config;
use lanstash_adapter_admin_api::api::health::HealthOf;
use lanstash_adapter_os::process::{Whose, open_files};
use lanstash_app::health::{self, Check, Level};
use lanstash_app::status::Status;

use super::{Loaded, dir_fs};

/// The checks, made when asked.
pub(super) fn checks(c: &Config, loaded: Loaded, status: &Arc<Status>) -> HealthOf {
    let status = Arc::clone(status);
    let (root, keep_free) = (c.store.clone(), c.keep_free);
    Arc::new(move || {
        let mut checks = Vec::new();
        let own = open_files(Whose::This);
        // The status's figures: walked again only once the stash changed (a walk of the whole
        // index takes seconds on a large stash).
        let snapshot = status.snapshot();
        let (games, bytes) = (snapshot.usage.len() as u64, snapshot.usage.iter().map(|r| r.usage.bytes).sum());
        let limit = own.as_ref().ok().map(|(_, l)| *l);
        checks.push(health::store_load(games, bytes, &loaded.failed, loaded.damaged, limit));
        checks.push(match &own {
            Ok((open, limit)) => health::open_files("lanstash", *open, *limit),
            Err(e) => Check::new(Level::Warn, "open files", format!("not known here: {e}")),
        });
        checks.push(match dir_fs(&root).and_then(|fs| fs.space().map_err(|e| e.to_string())) {
            Ok(Some(d)) => health::free_space(d.free, d.total, keep_free),
            Ok(None) => Check::new(Level::Warn, "free space", "unknown"),
            Err(e) => Check::new(Level::Warn, "free space", e),
        });
        if let Some(v) = snapshot.counters.iter().find(|g| g.name == "verify") {
            let get = |n: &str| v.values.iter().find(|(k, _)| *k == n).map_or(0, |(_, x)| *x);
            checks.push(health::background_check(get("problems"), get("finished_ms"), get("running") == 1));
        }
        checks
    })
}
