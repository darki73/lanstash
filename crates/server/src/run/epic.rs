//! Epic for the running service: its caching rules and setup file (the
//! platform itself), its accounts (signing in with the Epic launcher's own
//! client unless another is set from the account page), and the accounts'
//! library (read in the background, kept in the state directory): the games'
//! names, images and download sizes, the catalog view, the stashed games'
//! builds, prefill, and prune (by hand, and hourly on its own).

use std::sync::Arc;

use lanstash::config::Config;
use lanstash_adapter_fs::private_dir::PrivateDir;
use lanstash_adapter_http::web::HttpWeb;
use lanstash_adapter_os::pause::Sleep;
use lanstash_app::delete_game::GameDeleter;
use lanstash_app::prefill::{PrefillOptions, Prefiller};
use lanstash_app::prune_segments;
use lanstash_app::serve::{Cache, RequestHandler};
use lanstash_epic::auth::LAUNCHER_CLIENT;
use lanstash_epic::platform::EpicPlatform;
use lanstash_epic_app::accounts::{EpicAccounts, Wiring};
use lanstash_epic_app::builds::EpicBuilds;
use lanstash_epic_app::library::EpicLibrary;
use lanstash_epic_app::prefill::EpicPlanner;
use lanstash_epic_app::prune::EpicPruner;
use lanstash_epic_app::sizes::EpicSizes;
use lanstash_ports::clock::Clock;
use lanstash_ports::platform::ViewError;
use lanstash_ports::prefill::{Prefill, PrefillState};
use lanstash_ports::upstream::Upstream;
use lanstash_ports::web::Web;
use lanstash_store::store::Store;

/// Epic's service id.
const EPIC: &str = "epicgames";

pub(super) fn platform(
    c: &Config,
    clock: &Arc<dyn Clock>,
    cache: &Arc<Cache>,
    store: &Arc<Store>,
    up: &Arc<dyn Upstream>,
    deleter: &Arc<GameDeleter>,
) -> Result<EpicPlatform, String> {
    let web: Arc<dyn Web> = Arc::new(HttpWeb::default());
    let wiring = Wiring {
        web: Arc::clone(&web),
        secrets: Arc::new(PrivateDir::new(&c.secrets)),
        clock: Arc::clone(clock),
        client_id: LAUNCHER_CLIENT.id.to_string(),
        client_secret: LAUNCHER_CLIENT.secret.to_string(),
    };
    let accounts = Arc::new(EpicAccounts::new(wiring));
    let library = Arc::new(
        EpicLibrary::new(Arc::clone(&accounts), web, Arc::clone(clock)).with_keep(Arc::new(PrivateDir::new(&c.state))),
    );
    library.keep_fresh(Arc::new(Sleep))?;
    let view = Arc::clone(&library);
    // Chunks from the CDN are checked against the manifest's SHA-1 before they are stored.
    let planner = Arc::new(EpicPlanner::new(Arc::clone(&library), Arc::new(HttpWeb::default())));
    // The stashed games' builds: how much of each current build the stash holds.
    let builds = Arc::new(EpicBuilds::new(Arc::clone(&library), Arc::clone(&planner), Arc::clone(store)));
    builds.keep_fresh(Arc::new(Sleep))?;
    // Each library game's download size, worked out from its manifest one at a time and kept.
    let sizes = Arc::new(
        EpicSizes::new(Arc::clone(&library), Arc::clone(&planner)).with_keep(Arc::new(PrivateDir::new(&c.state))),
    );
    sizes.keep_going(Arc::new(Sleep))?;
    let pruner = Arc::new(EpicPruner::new(
        Arc::clone(&library),
        Arc::clone(&planner),
        Arc::clone(store),
        Arc::clone(clock),
        super::platforms::PRUNE_GRACE_MS,
    ));
    let client_path: Arc<dyn RequestHandler> = Arc::clone(cache) as Arc<dyn RequestHandler>;
    let opts = PrefillOptions { parallel: c.prefill_parallel, ..PrefillOptions::default() };
    let prefill = Arc::new(
        Prefiller::new(planner, EPIC, client_path, Arc::clone(store), Arc::clone(clock), opts)
            .with_pause(Arc::new(Sleep))
            .with_checked_cdn(Arc::clone(up), super::platforms::stash_put(c, store, EPIC)),
    );
    auto_prune(&pruner, &builds, deleter, &prefill);
    let (view_builds, status, mark) = (Arc::clone(&builds), Arc::clone(&builds), builds);
    let (find, parts) = (Arc::clone(&pruner), pruner);
    Ok(EpicPlatform::default()
        .with_builds(Arc::new(move || Ok(view_builds.view())), Arc::new(move || status.status()))
        .with_builds_mark(Arc::new(move || mark.builds_mark()))
        .with_prefill(prefill)
        .with_prune(Arc::new(move |game: &str| {
            find.find(game).map(|p| (prune_segments::view(game, &p), p.items)).map_err(ViewError::Unavailable)
        }))
        .with_parts(Arc::new(move |game: &str| Some(parts.dirs(game).into_iter().collect())))
        .with_account(accounts)
        .with_names(Arc::clone(&library) as _)
        .with_art(library as _)
        .with_catalog(Arc::new(move || Ok(view.view(&|a| sizes.size(a))))))
}

/// Old builds go on their own: hourly, the first game whose current build is stashed whole and
/// has segments no current build needs, unused for the grace period.
fn auto_prune(pruner: &Arc<EpicPruner>, builds: &Arc<EpicBuilds>, deleter: &Arc<GameDeleter>, jobs: &Arc<Prefiller>) {
    let (p, b, d, jobs) = (Arc::clone(pruner), Arc::clone(builds), Arc::clone(deleter), Arc::clone(jobs));
    let spawned = std::thread::Builder::new().name("epic-prune".into()).spawn(move || {
        loop {
            std::thread::sleep(super::platforms::AUTO_PRUNE_EVERY);
            let busy = |game: &str| jobs.status().is_some_and(|s| s.game == game && s.state == PrefillState::Running);
            let current = b.current();
            if let Some((game, bytes)) = prune_segments::prune_once(EPIC, &current, &|g| p.find(g), &d, &busy) {
                println!("pruning Epic game {game}: {:.2} GB no current build needs", bytes as f64 / 1e9);
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("automatic Epic pruning not started: {e}");
    }
}
