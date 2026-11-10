//! The commands: build adapters from the configuration, wire them into the use
//! cases, report. The only place that touches the real file system, network
//! and clock directly.

mod admin;
mod clients;
mod epic;
mod health;
mod kept;
mod platforms;
pub mod releases;
pub mod serve;
pub mod steam;
pub mod store;
pub mod users;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lanstash::cli::ConfigArgs;
use lanstash::config::{Config, Settings};
use lanstash_adapter_fs::dir_fs::DirFs;
use lanstash_adapter_http::server::{HttpServer, ServerConfig};
use lanstash_domain::http::Limits as HttpLimits;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::Clock;
use lanstash_ports::storage::StoreFs;
use lanstash_store::game::SegmentOptions;
use lanstash_store::store::{Roots, Store, StoreOptions};

/// Binds an HTTP server of `cfg`, its request heads timed by `clock`.
fn bind(cfg: ServerConfig, clock: &Arc<dyn Clock>) -> Result<HttpServer, String> {
    let addr = cfg.listen;
    Ok(HttpServer::bind(cfg).map_err(|e| format!("bind {addr}: {e}"))?.with_clock(Arc::clone(clock)))
}

/// The cache port's HTTP server settings (the admin port's start from them).
fn server_config(c: &Config) -> ServerConfig {
    ServerConfig {
        listen: c.listen,
        max_connections: c.max_connections,
        idle_timeout: Duration::from_secs(60),
        head_timeout: Duration::from_secs(15),
        max_per_client: (c.max_connections / 4).max(64), // a PC downloads over a few dozen
        write_timeout: Duration::from_secs(60),
        limits: HttpLimits::default(),
        thread_stack_bytes: 256 * 1024,
        zero_copy: true, // falls back by itself where the system or file system cannot
    }
}

/// Reads the configuration file (if any) and applies the flags over it.
pub fn config(args: &ConfigArgs) -> Result<Config, String> {
    let mut s = Settings::default();
    if let Some(f) = &args.file {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?;
        s.add_file(&f.display().to_string(), &text)?;
    }
    // The changes made in the web UI ([`lanstash::settings`]), before the flags.
    if let Some(state) = s.value("state").map(PathBuf::from) {
        for (k, v) in lanstash::settings::saved(&state) {
            s.set(&k, &v)?;
        }
    }
    for (k, v) in args.flags.pairs() {
        s.set(k, v)?;
    }
    s.build()
}

pub(crate) fn dir_fs(paths: &[PathBuf]) -> Result<Arc<dyn StoreFs>, String> {
    let (first, rest) = paths.split_first().ok_or("empty store path")?;
    let fs = DirFs::with_read_paths(first, rest).map_err(|e| e.to_string())?;
    Ok(Arc::new(fs))
}

pub(crate) fn store_roots(c: &Config) -> Result<Roots, String> {
    let mut per_service = HashMap::new();
    for (s, p) in &c.store_per_service {
        per_service.insert(s.clone(), dir_fs(p)?);
    }
    Ok(Roots { default: dir_fs(&c.store)?, per_service })
}

pub(super) fn open_store(c: &Config, clock: &Arc<dyn Clock>, read_only: bool) -> Result<(Store, Loaded), String> {
    let roots = store_roots(c)?;
    let segments = SegmentOptions { max_bytes: c.segment_size, ..SegmentOptions::default() };
    let opts = StoreOptions { segments, read_only };
    let started = Instant::now();
    let (store, r) = Store::open(roots, opts, Arc::clone(clock)).map_err(|e| e.to_string())?;
    let t = &r.totals;
    println!(
        "store loaded in {:.2}s: {} games; {} records ({:.2} GB) in {} sealed + {} open segments; \
         {} object files with {} extents ({:.2} GB)",
        started.elapsed().as_secs_f64(),
        r.games,
        t.records,
        t.record_bytes as f64 / 1e9,
        t.segments_sealed,
        t.segments_unsealed,
        t.object_files,
        t.extents,
        t.extent_bytes as f64 / 1e9
    );
    let notes = [
        (t.continued, "open segments continued"),
        (t.sealed_at_start, "open segments sealed"),
        (t.bad_bodies, "records dropped (body checksum failed)"),
        (t.legacy_slices, "v1 slice records skipped (v1 stores are not migrated)"),
        (t.journal_truncated, "journal bytes cut after a torn write"),
        (t.journal_skipped, "journal bytes of damaged entries skipped"),
        (t.journal_bad, "journal entries ignored"),
        (t.journals_compacted, "journals compacted"),
    ];
    for (n, what) in notes.iter().filter(|(n, _)| *n > 0) {
        println!("  {n} {what}");
    }
    for (path, err) in &t.failed {
        println!("  could not read {path}: {err}");
    }
    for s in &r.ignored {
        println!("  {s}: belongs to another root, not loaded");
    }
    Ok((store, Loaded { failed: t.failed.clone(), damaged: t.bad_bodies + t.journal_bad }))
}

/// What loading the store at the start could not take: files (path, error), damaged records and
/// journal entries (the health page says it).
#[derive(Debug, Clone, Default)]
pub(super) struct Loaded {
    pub failed: Vec<(String, String)>,
    pub damaged: u64,
}

/// Each cached service's cache-domains list: (service, its text).
pub(crate) fn host_lists(c: &Config) -> Result<Vec<(String, String)>, String> {
    let Some(dir) = &c.cache_domains else { return Ok(Vec::new()) };
    c.services
        .iter()
        .map(|s| {
            let path = dir.join(format!("{s}.txt"));
            std::fs::read_to_string(&path).map(|t| (s.clone(), t)).map_err(|e| format!("{}: {e}", path.display()))
        })
        .collect()
}

/// The host rules, and per service how many host patterns its list added.
pub(crate) fn host_rules(c: &Config) -> Result<(HostRules, Vec<(String, usize)>), String> {
    let mut rules = HostRules::default();
    // Steam's content hosts: lancache's list names only the discovery name.
    for host in lanstash_steam_domain::profile::CONTENT_HOSTS {
        rules.add_host("steam", host);
    }
    // Epic's content hosts its list lacks, where Epic is cached.
    if c.services.iter().any(|s| s == "epicgames") {
        for host in lanstash_epic::profile::CONTENT_HOSTS {
            rules.add_host("epicgames", host);
        }
    }
    let counts = host_lists(c)?.into_iter().map(|(s, text)| (s.clone(), rules.add_list(&s, &text))).collect();
    Ok((rules, counts))
}
