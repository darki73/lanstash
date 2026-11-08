//! `lanstash serve`: the long-running server, its eviction thread and its
//! clean stop; `lanstash stop` asks a running one to stop.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use lanstash::cli::ConfigArgs;
use lanstash::config::Config;
use lanstash::dns::{LiveDns, patterns};
use lanstash::report::Outcome;
use lanstash::settings::LiveSettings;
use lanstash_adapter_admin_api::api::{AdminApi, StopHook};
use lanstash_adapter_admin_api::client;
use lanstash_adapter_dns::resolver::{Resolve, ResolverConfig, UdpResolver};
use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_adapter_http::tls_passthrough::{TlsConfig, TlsCounters, TlsPassthrough};
use lanstash_adapter_http::upstream::{HttpUpstream, UpstreamConfig};
use lanstash_adapter_http::web::HttpWeb;
use lanstash_adapter_logs::access_log::FileAccessLog;
use lanstash_adapter_logs::event_log::EventLog;
use lanstash_adapter_logs::rotating_file::Rotation;
use lanstash_adapter_names::game_names::ChainNames;
use lanstash_adapter_os::pause::Sleep;
use lanstash_app::art::{Art, ArtOptions};
use lanstash_app::auto_update::AutoUpdate;
use lanstash_app::build_history::BuildHistory;
use lanstash_app::delete_game::GameDeleter;
use lanstash_app::dns::DnsLink;
use lanstash_app::evict::{EvictOptions, Evictor};
use lanstash_app::fetch::{FetchOptions, Fetcher};
use lanstash_app::hidden::HiddenGames;
use lanstash_app::lifetime::Lifetime;
use lanstash_app::passthrough::Passthrough;
use lanstash_app::patrol::{Patrol, PatrolOptions};
use lanstash_app::private_games::PrivateGames;
use lanstash_app::serve::{Cache, RequestHandler, ServeOptions};
use lanstash_app::shutdown::Shutdown;
use lanstash_app::status::{Status, USAGE_MAX_AGE_MS};
use lanstash_domain::evict::{KeepFree, Limits};
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::Clock;
use lanstash_ports::events::Events;
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::Platform;
use lanstash_ports::upstream::Upstream;
use lanstash_riot::platform::RiotPlatform;
use lanstash_store::store::Store;

use super::platforms::{battle_net, game_names, kept_builds, profiles};
use super::{config, host_rules, open_store};

pub fn serve(args: &ConfigArgs, clock: &Arc<dyn Clock>) -> Result<Outcome, String> {
    let c = config(args)?;
    if c.upstream_dns.is_empty() {
        return Err("`upstream_dns` is required to serve".into());
    }
    let (store, loaded) = open_store(&c, clock, false).map(|(s, l)| (Arc::new(s), l))?;
    let events: Arc<dyn Events> = match &c.event_log {
        Some(p) => Arc::new(
            EventLog::open(p, rotation(&c), Arc::clone(clock))
                .map_err(|e| format!("event log {}: {e}", p.display()))?,
        ),
        None => Arc::new(EventLog::stderr(Arc::clone(clock))),
    };
    let resolver: Arc<dyn Resolve> = Arc::new(UdpResolver::new(
        ResolverConfig { servers: c.upstream_dns.clone(), ..ResolverConfig::default() },
        Arc::clone(clock),
    ));
    let upstream = Arc::new(HttpUpstream::new(Arc::clone(&resolver), UpstreamConfig::default()));
    let fetch_opts = FetchOptions {
        piece: c.fetch_piece,
        ahead: c.fetch_ahead,
        window: c.download_window,
        object_file_min: c.object_file_min,
        ..FetchOptions::default()
    };
    let (rules, counts) = host_rules(&c)?;
    let fetcher = Fetcher::new(upstream.clone(), Arc::clone(&store), Arc::clone(&events), fetch_opts);
    let fetcher = Arc::new(fetcher.with_hosts(rules.clone()).with_clock(Arc::clone(clock)));
    let up: Arc<dyn Upstream> = upstream.clone();
    let pass = Arc::new(Passthrough::new(upstream, fetch_opts.max_unknown_length, fetch_opts.read_bytes));
    for (s, n) in counts {
        println!("cache-domains: {s}: {n} host patterns");
    }
    println!("services: {}", rules.services().join(", "));
    let serve_opts = ServeOptions { piece_bytes: c.send_piece, prefetch: c.prefetch, retries: c.retries };
    let tls = match c.listen_tls {
        Some(addr) => {
            let cfg = TlsConfig { max_connections: c.max_connections, ..TlsConfig::new(addr) };
            let t = TlsPassthrough::bind(cfg, rules.clone(), resolver, Arc::clone(&events), Arc::clone(clock))
                .map_err(|e| format!("bind {addr}: {e}"))?;
            println!("passing HTTPS through on {}", t.local_addr().map_err(|e| e.to_string())?);
            Some(t)
        }
        None => None,
    };
    let tls_counters = tls.as_ref().map(|t| Arc::clone(&t.counters));
    if let Some(t) = tls {
        std::thread::Builder::new()
            .name("lanstash-tls-accept".into())
            .spawn(move || t.run())
            .map_err(|e| e.to_string())?;
    }
    let evictor = start_eviction(&c, &store, &events, clock)?;
    let cache = Arc::new(
        Cache::new(Arc::clone(&store), rules.clone(), &profiles(), Arc::clone(&events), serve_opts)
            .with_upstream(fetcher, pass)
            .with_clients(super::clients::traffic(clock)),
    );
    if c.admin_listen.is_some() {
        let deleter = Arc::new(GameDeleter::new(Arc::clone(&store), Arc::clone(&events), Arc::clone(clock)));
        let platforms: Vec<Arc<dyn Platform>> = vec![
            Arc::new(super::steam::platform(&c, &cache, &store, &up, clock, &deleter)?),
            Arc::new(battle_net(&c, &cache, &store, &up, clock, &deleter)),
            Arc::new(super::epic::platform(&c, clock, &cache, &store, &up, &deleter)?),
            Arc::new(RiotPlatform),
        ];
        let db = super::kept::database(&c)?;
        let history = Arc::new(BuildHistory::new(Arc::clone(&db) as _));
        let platforms = kept_builds(platforms, &store, clock, &history)?;
        let mut art = Art::new(Arc::clone(&up), Arc::clone(clock), ArtOptions::default());
        for p in &platforms {
            if let Some(source) = p.art() {
                art = art.with_source(p.id(), source);
            }
        }
        // The platforms' own names first (exact once read), then the names file and the grouping rules.
        let mut names: Vec<Arc<dyn GameNames>> = platforms.iter().filter_map(|p| p.names()).collect();
        names.push(game_names(&c)?);
        let names: Arc<dyn GameNames> = Arc::new(ChainNames(names));
        let lifetime = Arc::new(Lifetime::new(Arc::clone(&cache), Arc::new(PrivateDir::new(&c.state)), clock.now_ms()));
        lifetime.keep_going(Arc::new(Sleep), LIFETIME_SAVE_EVERY_MS)?;
        let status = Status::new(Arc::clone(&cache), Arc::clone(&store), Arc::clone(clock), names, USAGE_MAX_AGE_MS);
        let status = status.with_usage_min_age(USAGE_MIN_AGE_MS);
        let mut status = status.with_evictor(Arc::clone(&evictor)).with_lifetime(Arc::clone(&lifetime));
        if let Some(p) = start_patrol(&c, clock)? {
            status = status.with_patrol(p);
        }
        let status = Arc::new(status);
        let more = super::kept::https(tls_counters.as_ref());
        // Games once held whole are queued for their new builds inside the window (not once deleted).
        let auto = match c.auto_update {
            Some(window) => {
                let auto = AutoUpdate::new(platforms.clone(), Arc::clone(clock), window)
                    .with_keep(Arc::new(PrivateDir::new(&c.state)))
                    .with_deleter(Arc::clone(&deleter));
                let auto = Arc::new(auto);
                auto.keep_going(Arc::new(Sleep), AUTO_UPDATE_EVERY_MS)?;
                Some(auto)
            }
            None => None,
        };
        let (sessions, invites) = super::users::sessions(&c, clock)?;
        let mut api = AdminApi::new(Arc::clone(&status), Duration::from_secs(1))
            .with_stop(stop_hook(&store, &lifetime, 0))
            .with_settings(
                Arc::new(LiveSettings::new(args.file.clone(), args.flags.clone(), &c.state)),
                // A clean stop with the restart code: the service manager starts lanstash again.
                stop_hook(&store, &lifetime, RESTART_CODE),
            )
            .with_art(Arc::new(art))
            .with_prefill_runs(super::kept::prefill_runs(&db, &platforms)?)
            .with_stats(super::kept::counters(&db, &status, more.clone(), clock)?)
            .with_platforms(platforms)
            .with_history(history)
            .with_health(super::health::checks(&c, loaded, &status))
            .with_deleter(deleter)
            .with_clients(super::clients::names(&c, &db, &cache, clock)?)
            .with_hidden(Arc::new(HiddenGames::new(Arc::new(PrivateDir::new(&c.state)))?))
            .with_private(Arc::new(PrivateGames::new(Arc::new(PrivateDir::new(&c.state)))?))
            .with_sessions(sessions)
            .with_invites(invites)
            .with_dns(Arc::new(dns_link(&c, &cache, rules)?));
        for (name, counters) in more {
            api = api.with_counters(name, counters);
        }
        if let Some(auto) = auto {
            api = api.with_auto_update(auto);
        }
        if let Some(dir) = c.web_ui.clone() {
            api = api.with_web(dir);
        }
        super::admin::start(&c, Arc::new(api), clock)?;
    }

    let server = super::bind(super::server_config(&c), clock)?;
    let server = match &c.access_log {
        Some(path) => {
            let log = FileAccessLog::open(path, rotation(&c), Arc::clone(clock))
                .map_err(|e| format!("access log {}: {e}", path.display()))?
                .with_all_headers(c.access_log_headers);
            server.with_access_log(Arc::new(log))
        }
        None => server,
    };
    println!("serving on http://{}", server.local_addr().map_err(|e| e.to_string())?);
    if c.stats_secs > 0 {
        let cache = Arc::clone(&cache);
        let every = Duration::from_secs(c.stats_secs);
        std::thread::Builder::new()
            .name("lanstash-stats".into())
            .spawn(move || stats_loop(&cache, tls_counters.as_deref(), every))
            .map_err(|e| e.to_string())?;
    }
    let handler: Arc<dyn RequestHandler> = cache;
    server.run(&handler);
    Ok(Outcome::Ok)
}

/// How often the background store check looks whether a round is due (the first time too).
const PATROL_LOOK_EVERY_MS: u64 = 10 * 60_000;

/// The background store check, when one is set (`verify_every`).
fn start_patrol(c: &Config, clock: &Arc<dyn Clock>) -> Result<Option<Arc<Patrol>>, String> {
    let Some(days) = c.verify_every_days else { return Ok(None) };
    let roots = super::store_roots(c)?;
    let plan = Box::new(move || lanstash_store::verify::plan(&roots).map_err(|e| e.to_string()));
    let options = PatrolOptions { every_ms: days * 86_400_000, rate: c.verify_rate };
    let keep = Arc::new(PrivateDir::new(&c.state));
    let patrol = Arc::new(Patrol::new(plan, Arc::clone(clock), Arc::new(Sleep), keep, options));
    patrol.keep_going(PATROL_LOOK_EVERY_MS)?;
    Ok(Some(patrol))
}

fn stats_loop(cache: &Cache, tls: Option<&TlsCounters>, every: Duration) {
    let g = |x: &AtomicU64| x.load(Ordering::Relaxed);
    let mut last = [0u64; 3];
    loop {
        std::thread::sleep(every);
        let (s, f, p) =
            (&cache.counters, &cache.fetcher().expect("wired").counters, &cache.passthrough().expect("wired").counters);
        let now = [g(&s.hit_bytes), g(&s.miss_bytes), g(&f.fetched_bytes)];
        let rate = |i: usize| (now[i] - last[i]) as f64 / 1e6 / every.as_secs_f64();
        println!(
            "  requests {} | hits {} {:.1} MB/s | misses {} {:.1} MB/s | origin {:.1} MB/s, downloads {} in flight {} \
             failed {} crc {} range-ignored {} | stored {} records {} extents | pass {} | errors {} store {} retries {}",
            g(&s.requests),
            g(&s.hits),
            rate(0),
            g(&s.misses),
            rate(1),
            rate(2),
            g(&f.started),
            cache.fetcher().map_or(0, |f| f.in_flight()),
            g(&f.failed),
            g(&f.crc_mismatch),
            g(&f.range_ignored),
            g(&f.stored_records),
            g(&f.stored_extents),
            g(&p.requests),
            g(&s.errors),
            g(&f.store_errors),
            g(&s.retries),
        );
        if let Some(t) = tls {
            println!(
                "  https: connections {} active {} relayed {} refused {} bad-hello {} connect-failed {} | up {:.1} MB down {:.1} MB",
                g(&t.connections),
                t.active.load(Ordering::Relaxed),
                g(&t.relayed),
                g(&t.refused),
                g(&t.bad_hello),
                g(&t.connect_failed),
                g(&t.bytes_up) as f64 / 1e6,
                g(&t.bytes_down) as f64 / 1e6
            );
        }
        last = now;
    }
}

/// How often the last-use times are written (lost on a crash: at most this much).
const RECENCY_SAVE_EVERY: Duration = Duration::from_secs(300);

/// Time between automatic update passes.
const AUTO_UPDATE_EVERY_MS: u64 = 10 * 60 * 1000;

/// lanstash and the network's DNS server, as the web UI sets it up: the setup in the state
/// directory, the password among the secrets, its admin API reached over HTTP(S); the names wanted
/// are the cached services' lists.
fn dns_link(c: &Config, cache: &Arc<Cache>, rules: HostRules) -> Result<DnsLink, String> {
    let lists = super::host_lists(c)?;
    let texts: Vec<&str> = lists.iter().map(|(_, t)| t.as_str()).collect();
    // The lists as written, and Epic's built-in content hosts (its launcher asks each by its
    // own name). Not Steam's: a Steam client that finds `lancache.steamcontent.com` pointing at
    // a cache sends all its downloads there, whatever CDN name it asks for, so those need no rewrite.
    let epic: &[&str] =
        if c.services.iter().any(|s| s == "epicgames") { lanstash_epic::profile::CONTENT_HOSTS } else { &[] };
    let wanted = patterns(&texts, epic);
    let (state, secrets) = (Arc::new(PrivateDir::new(&c.state)), Arc::new(PrivateDir::new(&c.secrets)));
    let setup = LiveDns::new(state, secrets, Arc::new(HttpWeb::default()));
    Ok(DnsLink::new(Arc::new(setup), Arc::clone(cache), rules, wanted))
}

/// How often the lifetime totals are saved (and so at most what a crash loses).
const LIFETIME_SAVE_EVERY_MS: u64 = 60 * 1000;

/// The shortest time between two walks of the per-game usage while the stash keeps changing.
const USAGE_MIN_AGE_MS: u64 = 5_000;

/// Starts the eviction thread: one pass every `evict_secs`, the last-use
/// times saved every [`RECENCY_SAVE_EVERY`].
fn start_eviction(
    c: &Config,
    store: &Arc<Store>,
    events: &Arc<dyn Events>,
    clock: &Arc<dyn Clock>,
) -> Result<Arc<Evictor>, String> {
    let limits = Limits { max_size: c.max_size, keep_free: c.keep_free };
    let opts = EvictOptions { limits, ..EvictOptions::default() };
    let evictor = Arc::new(Evictor::new(Arc::clone(store), Arc::clone(events), Arc::clone(clock), opts));
    let show = |b: Option<u64>| b.map_or_else(|| "off".to_owned(), |b| format!("{:.0} GB", b as f64 / 1e9));
    let keep = match c.keep_free {
        Some(KeepFree::Percent(p)) => format!("{p}%"),
        Some(KeepFree::Bytes(b)) => show(Some(b)),
        None => "off".to_owned(),
    };
    println!("eviction: max_size {}, keep_free {keep}, every {} s", show(c.max_size), c.evict_secs);
    let (e, store, every) = (Arc::clone(&evictor), Arc::clone(store), Duration::from_secs(c.evict_secs));
    let mut saved = Instant::now();
    lanstash_app::rounds::forever("lanstash-evict", Arc::new(Sleep), move || {
        std::thread::sleep(every);
        let p = e.run_once();
        if !p.evicted.is_empty() || !p.failed.is_empty() {
            let bytes: u64 = p.evicted.iter().map(|(_, b)| b).sum();
            println!(
                "  evicted {} items, {:.2} GB ({} failed); over max_size {:.2} GB, short of keep_free {:.2} GB",
                p.evicted.len(),
                bytes as f64 / 1e9,
                p.failed.len(),
                p.over_max_size as f64 / 1e9,
                p.short_of_free.iter().max().copied().unwrap_or(0) as f64 / 1e9
            );
        }
        if saved.elapsed() >= RECENCY_SAVE_EVERY || !p.evicted.is_empty() {
            for (game, err) in store.save_recency() {
                println!("  saving last-use times of {}: {err}", game.path());
            }
            saved = Instant::now();
        }
        lanstash_app::rounds::Next::Again
    })?;
    Ok(evictor)
}

fn rotation(c: &Config) -> Rotation {
    Rotation { max_bytes: c.log_max_size, keep: c.log_keep }
}

/// How long a clean stop waits for records still being written.
const STOP_WAIT: Duration = Duration::from_secs(20);

/// `POST /shutdown`: seal the segments, save the last-use times, exit.
/// The exit code of a stop for a restart (settings changed in the web UI):
/// not 0, so a service restarted on failure (`lanstash install`) starts again.
pub const RESTART_CODE: i32 = 75;

/// Stops cleanly and ends the process with `code`.
fn stop_hook(store: &Arc<Store>, lifetime: &Arc<Lifetime>, code: i32) -> StopHook {
    let store = Arc::clone(store);
    let lifetime = Arc::clone(lifetime);
    Arc::new(move || {
        println!("stopping: sealing segments and saving last-use times …");
        if let Err(e) = lifetime.save() {
            println!("  saving the lifetime totals: {e}");
        }
        let s = Shutdown::begin(Arc::clone(&store));
        let step = Duration::from_millis(100);
        let mut waited = Duration::ZERO;
        while s.open() > 0 && waited < STOP_WAIT {
            std::thread::sleep(step);
            waited += step;
        }
        let r = s.finish();
        println!(
            "stopped: {} segments sealed, {} left open (recovered at the next start)",
            r.segments_sealed, r.segments_open
        );
        for (game, e) in &r.recency_failed {
            println!("  saving last-use times of {game}: {e}");
        }
        std::process::exit(code);
    })
}

/// `lanstash stop`: asks the running lanstash (on `admin_listen`) to stop cleanly.
pub fn stop(args: &ConfigArgs, wait: Duration) -> Result<Outcome, String> {
    let c = config(args)?;
    let addr = c.admin_listen.ok_or("`admin_listen` is not set: lanstash can only be stopped through it")?;
    client::stop(addr, wait)?;
    println!("lanstash stopped");
    Ok(Outcome::Ok)
}
