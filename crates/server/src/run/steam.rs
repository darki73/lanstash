//! `lanstash steam login|status|logout`: the Steam app's login and logon,
//! wired to the WebSocket adapter, the terminal, ring's randomness and the
//! secrets directory. Runs in the command's own process (run as root,
//! the token stays root-only); the running service is not involved.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use lanstash::cli::{ConfigArgs, SteamAction};
use lanstash::config::Config;
use lanstash::report::Outcome;
use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_adapter_os::pause::Sleep;
use lanstash_adapter_os::terminal::Terminal;
use lanstash_adapter_steam::link::WssDialer;
use lanstash_adapter_steam::random::RingRandom;
use lanstash_domain::qr::encode;
use lanstash_domain::qr::render::text;
use lanstash_steam_app::account::{SteamAccount, Wiring};
use lanstash_steam_app::connection::Connection;
use lanstash_steam_app::login::{Login, accounts, forget, login};
use lanstash_steam_app::logon::logon;
use lanstash_steam_app::qr_login::{QrLogin, Step};

use lanstash_adapter_fs::nginx_dir::NginxDir;
use lanstash_app::delete_game::GameDeleter;
use lanstash_app::prefill::{PrefillOptions, Prefiller};
use lanstash_app::prune_segments;
use lanstash_app::serve::{Cache, RequestHandler};
use lanstash_ports::clock::Clock;
use lanstash_ports::keep::Keep;
use lanstash_ports::link::Dialer;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::platform::ViewError;
use lanstash_ports::prefill::{Prefill, PrefillState};
use lanstash_ports::secrets::Secrets;
use lanstash_ports::upstream::Upstream;
use lanstash_steam_app::builds::SteamBuilds;
use lanstash_steam_app::inspect::LancacheInspect;
use lanstash_steam_app::lancache::SteamLancache;
use lanstash_steam_app::library::SteamLibrary;
use lanstash_steam_app::prefill::SteamPlanner;
use lanstash_steam_app::prune::{self, SteamPruner};
use lanstash_steam_app::session::Services;
use lanstash_steam_domain::apps::{App, Target};
use lanstash_steam_domain::art::SteamArt;
use lanstash_steam_domain::catalog;
use lanstash_steam_domain::platform::{ID as STEAM, SteamPlatform};
use lanstash_store::store::Store;

use super::config;
use super::platforms::{lancache_read, stash_put};

/// How often the shared Steam session sends a heartbeat (Steam asks for one every 9 s).
const HEARTBEAT_MS: u64 = 9_000;

/// The installed configuration, used when no `--config` is given.
const INSTALLED: &str = "/etc/lanstash/lanstash.conf";

/// Connects and every read waits at most this long; a person typing a code
/// is not waited on by the link (the prompt runs between reads).
const TIMEOUT: Duration = Duration::from_secs(10);

/// This machine's name in the account's device list.
fn device() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .map_or_else(|| "lanstash".to_string(), |h| format!("lanstash ({h})"))
}

pub fn steam(args: &ConfigArgs, action: SteamAction) -> Result<Outcome, String> {
    let mut args = args.clone();
    if args.file.is_none() && Path::new(INSTALLED).exists() {
        args.file = Some(INSTALLED.into());
    }
    let c = config(&args)?;
    let secrets = PrivateDir::new(&c.secrets);
    let dialer = WssDialer { timeout: TIMEOUT };
    match action {
        SteamAction::Login { password } => {
            let mut conn = Connection::open(&dialer)?;
            let device = device();
            let name = if password {
                let with = Login { prompt: &Terminal, random: &RingRandom, secrets: &secrets, device: &device };
                login(&mut conn, &with)?
            } else {
                qr(&mut conn, &secrets, &device)?
            };
            println!("Signed in to Steam as {name}; the refresh token is kept in {}.", c.secrets.display());
            let on = logon(&mut conn, &secrets, &name)?;
            println!("Logged on: steamid {}, cell {}.", on.steamid, on.cell_id);
        }
        SteamAction::Status => {
            let names = accounts(&secrets)?;
            if names.is_empty() {
                println!("Steam: no account signed in.");
            }
            for name in names {
                let mut conn = Connection::open(&dialer)?;
                match logon(&mut conn, &secrets, &name) {
                    Ok(on) => {
                        println!("Steam: logged on as {} (steamid {}, cell {}).", on.account, on.steamid, on.cell_id)
                    }
                    Err(e) => println!("Steam: {name}: {e}"),
                }
            }
        }
        SteamAction::Logout => {
            for name in accounts(&secrets)? {
                forget(&secrets, &name)?;
                println!(
                    "Steam: {name}'s kept token is forgotten (Steam itself still lists the device until it is removed there)."
                );
            }
        }
    }
    Ok(Outcome::Ok)
}

/// Steam for the running service: its account (the QR sign-in from the web UI),
/// its library (read in the background once signed in: names and what can be
/// prefilled), and prefill through the cache clients use (from lancache's old
/// cache too, when `nginx_cache` is set).
pub(super) fn platform(
    c: &Config,
    cache: &Arc<Cache>,
    store: &Arc<Store>,
    up: &Arc<dyn Upstream>,
    clock: &Arc<dyn Clock>,
    deleter: &Arc<GameDeleter>,
) -> Result<SteamPlatform, String> {
    let dialer: Arc<dyn Dialer> = Arc::new(WssDialer { timeout: TIMEOUT });
    let secrets: Arc<dyn Secrets> = Arc::new(PrivateDir::new(&c.secrets));
    // Who an install is for (the configured OS, language and architecture).
    let target = Target { os: c.steam_os.clone(), language: c.steam_language.clone(), arch: c.steam_arch.clone() };
    // Manifests' chunk lists and the library are kept in the state directory between runs,
    // so a restart downloads only new builds' manifests.
    let state: Arc<dyn Keep> = Arc::new(PrivateDir::new(&c.state));
    let services = Services::new(Arc::clone(&dialer), Arc::clone(&secrets), Arc::clone(up), Arc::clone(clock))
        .with_chunk_lists(Arc::clone(&state));
    let refused = services.clone();
    let counted = services.clone();
    let dropped = services.clone();
    services.keep_alive(Arc::new(Sleep), HEARTBEAT_MS)?;
    let wiring = Wiring {
        dialer,
        secrets,
        pause: Arc::new(Sleep),
        device: device(),
        refused: Arc::new(move |account: &str| refused.token_refused(account)),
        signed_out: Arc::new(move |account: &str| dropped.forget_session(account)),
        logons: Arc::new(move || counted.logons()),
    };
    let library = Arc::new(SteamLibrary::new(services.clone(), Arc::clone(clock)).with_keep(Arc::clone(&state)));
    library.keep_fresh(Arc::new(Sleep))?;
    // The stashed apps' builds go first on the shared session after a start; the probe waits.
    let builds_ready = Arc::new(AtomicBool::new(false));
    let shelf = Arc::clone(&library);
    let owners = Arc::clone(&library);
    let (forgotten, lists) = (Arc::clone(&library), services.lists.clone());
    // lancache's old cache, read-only: a prefill source, and how much of each game it holds.
    let source: Option<Arc<dyn NginxCacheSource>> =
        c.nginx_cache.as_ref().map(|dir| Arc::new(NginxDir::new(dir)) as Arc<dyn NginxCacheSource>);
    let probe = match &source {
        Some(s) => {
            let p = SteamLancache::new(services.clone(), Arc::clone(&library), Arc::clone(s), target.clone())
                .with_keep(Arc::clone(&state))
                .after(Arc::clone(&builds_ready));
            let p = Arc::new(p);
            p.keep_fresh(Arc::new(Sleep))?;
            Some(p)
        }
        None => None,
    };
    let mut builds =
        SteamBuilds::new(services.clone(), Arc::clone(&library), Arc::clone(store), target.clone(), source.is_some())
            .with_ready(builds_ready);
    if let Some(p) = &probe {
        builds = builds.with_probe(Arc::clone(p));
    }
    let builds = Arc::new(builds);
    builds.keep_fresh(Arc::new(Sleep))?;
    let pruner = Arc::new(SteamPruner::new(
        services.clone(),
        Arc::clone(&library),
        Arc::clone(store),
        Arc::clone(clock),
        super::platforms::PRUNE_GRACE_MS,
    ));
    let planner = Arc::new(SteamPlanner::new(services.clone(), Arc::clone(&library), target.clone()));
    let client_path: Arc<dyn RequestHandler> = Arc::clone(cache) as Arc<dyn RequestHandler>;
    let opts = PrefillOptions { parallel: c.prefill_parallel, ..PrefillOptions::default() };
    let mut prefill = Prefiller::new(planner, STEAM, client_path, Arc::clone(store), Arc::clone(clock), opts)
        .with_pause(Arc::new(Sleep));
    let inspect = source.as_ref().map(|s| {
        let read = lancache_read(Arc::clone(s), STEAM);
        Arc::new(LancacheInspect::new(services.clone(), Arc::clone(&library), Arc::clone(s), read, target.clone()))
    });
    if let Some(source) = source {
        prefill =
            prefill.with_lancache(lancache_read(source, STEAM), stash_put(c, store, STEAM)).with_tails(Arc::clone(up));
    }
    let prefill = Arc::new(prefill);
    // Old builds go on their own: hourly, the first app whose current build is stashed
    // whole and has segments no current build needs, unused for the grace period.
    let (p, b, d, jobs) = (Arc::clone(&pruner), Arc::clone(&builds), Arc::clone(deleter), Arc::clone(&prefill));
    let spawned = std::thread::Builder::new().name("steam-prune".into()).spawn(move || {
        loop {
            std::thread::sleep(super::platforms::AUTO_PRUNE_EVERY);
            let busy = |game: &str| jobs.status().is_some_and(|s| s.game == game && s.state == PrefillState::Running);
            if let Some((game, bytes)) = prune::prune_once(&p, &b, &d, &busy) {
                println!("pruning Steam app {game}: {:.2} GB no current build needs", bytes as f64 / 1e9);
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("automatic Steam pruning not started: {e}");
    }
    // Art by the file names app info gives (newer apps keep theirs under a hash).
    let shelf_art = Arc::clone(&library);
    let art = SteamArt::default()
        .with_assets(Arc::new(move |app: u32| shelf_art.get().and_then(|l| l.app(app).map(|a| a.art.clone()))));
    let mut platform = SteamPlatform::default().with_art(art);
    if let Some(i) = inspect {
        platform = platform.with_nginx(Arc::new(move |game: &str| i.inspect(game).map_err(ViewError::Unavailable)));
    }
    Ok(platform
        .with_account(Arc::new(SteamAccount::new(wiring)))
        .with_prefill(prefill)
        .with_status({
            let b = Arc::clone(&builds);
            Arc::new(move || b.status())
        })
        .with_builds_mark({
            let b = Arc::clone(&builds);
            Arc::new(move || b.builds_mark())
        })
        .with_builds(Arc::new(move || Ok(builds.view())))
        .with_prune(Arc::new(move |game: &str| {
            pruner.find(game).map(|p| (prune_segments::view(game, &p), p.items)).map_err(ViewError::Unavailable)
        }))
        .with_parts(Arc::new(move |game: &str| {
            // An app of the library: itself (a directory of the same number may hold some) and its depots.
            let lib = owners.get()?;
            let app: u32 = game.parse().ok()?;
            lib.app(app)?;
            let depots = lib.depot_app.iter().filter(|(_, a)| **a == app).map(|(d, _)| d.to_string());
            Some(std::iter::once(game.to_string()).chain(depots).collect())
        }))
        .with_forget(Arc::new(move |game: &str| {
            // Its depots' kept chunk lists (a new download of the game reads its manifests anew).
            let (Some(lib), Ok(app)) = (forgotten.get(), game.parse::<u32>()) else { return };
            for (d, _) in lib.depot_app.iter().filter(|(_, a)| **a == app) {
                lists.forget(*d);
            }
        }))
        .with_catalog(Arc::new(move || {
            // Until the library is read: no games, and why.
            let Some(lib) = shelf.get() else {
                let why = shelf.error().unwrap_or_else(|| "reading the Steam library".to_string());
                return Ok(catalog::render_none(&why));
            };
            let apps = lib.apps.clone();
            let share = |a: &App| probe.as_ref().and_then(|p| p.share(&lib, a));
            // What a prefill downloads: own depots, owned DLCs' and shared ones.
            let size = |a: &App| lib.install(a, &target).iter().map(|(_, d)| d.download).sum();
            let owners = |a: &App| lib.app_owners.get(&a.id).cloned().unwrap_or_default();
            Ok(catalog::render(&apps, &target, &size, &share, &owners))
        }))
        .with_names(library))
}

/// Polls before a QR code is given up on (about five minutes at Steam's 5 s).
const QR_POLLS: u32 = 60;

/// Signs in by QR code on the terminal: shows the code (again when Steam
/// rotates it) and waits for the approval in the Steam mobile app.
fn qr(conn: &mut Connection, secrets: &PrivateDir, device: &str) -> Result<String, String> {
    let mut q = QrLogin::start(conn, device)?;
    let (mut shown, mut scanned) = (String::new(), false);
    for _ in 0..QR_POLLS {
        if q.url != shown {
            let code = encode(q.url.as_bytes(), None).ok_or("the sign-in link is too long for a QR code")?;
            println!("{}", text(&code));
            println!("Scan this with the Steam mobile app (Steam Guard, then the QR scanner), then approve.");
            shown.clone_from(&q.url);
        }
        std::thread::sleep(Duration::from_millis(q.interval_ms()));
        if let Step::Done(name) = q.poll(conn, secrets)? {
            return Ok(name);
        }
        if q.scanned && !scanned {
            println!("Scanned; approve the sign-in in the app.");
            scanned = true;
        }
    }
    Err("the QR code was not approved in time; run `lanstash steam login` again".into())
}
