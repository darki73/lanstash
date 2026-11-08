//! The platforms wired from the configuration: their names, profiles and
//! use cases.

use std::sync::Arc;

use lanstash::config::Config;
use lanstash_adapter_fs::nginx_dir::NginxDir;
use lanstash_adapter_names::game_names::{ChainNames, TableNames};
use lanstash_adapter_os::pause::Sleep;
use lanstash_app::build_history::BuildHistory;
use lanstash_app::delete_game::GameDeleter;
use lanstash_app::kept_builds::{KeepOptions, KeptBuilds};
use lanstash_app::lancache_read::{LancacheOptions, LancacheRead};
use lanstash_app::prefill::{PrefillOptions, Prefiller};
use lanstash_app::serve::{Cache, RequestHandler};
use lanstash_app::stash_put::StashPut;
use lanstash_battlenet_api::platform::{BattleNet, ID as BATTLENET};
use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildOptions};
use lanstash_battlenet_app::catalog::{BlizzardCatalog, CatalogOptions, CodeOf};
use lanstash_battlenet_app::nginx::{NginxProbe, ProbeOptions};
use lanstash_battlenet_app::plans::{BlizzardPlans, PlanOptions};
use lanstash_battlenet_app::prefill::BlizzardJobs;
use lanstash_battlenet_app::products::{BlizzardProducts, ProductList, ProductOptions};
use lanstash_battlenet_app::prune::{Pruner, prune_once};
use lanstash_battlenet_domain::profile::Blizzard;
use lanstash_domain::names::NameTable;
use lanstash_domain::service::Profiles;
use lanstash_epic::profile::Epic;
use lanstash_ports::clock::Clock;
use lanstash_ports::names::GameNames;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_ports::platform::Platform;
use lanstash_ports::prefill::{Prefill, PrefillState};
use lanstash_ports::upstream::Upstream;
use lanstash_riot::profile::Riot;
use lanstash_steam_domain::names::DEPOT_WINDOW;
use lanstash_steam_domain::platform::ID as STEAM_ID;
use lanstash_steam_domain::profile::Steam;
use lanstash_store::store::Store;

/// The platforms with their builds worked out on a thread each and kept: asking costs
/// nothing, a slow platform holds back no other, the status stream says when they change,
/// and `history` notes what changed.
pub(super) fn kept_builds(
    platforms: Vec<Arc<dyn Platform>>,
    store: &Arc<Store>,
    clock: &Arc<dyn Clock>,
    history: &Arc<BuildHistory>,
) -> Result<Vec<Arc<dyn Platform>>, String> {
    platforms
        .into_iter()
        .map(|p| {
            let kept = KeptBuilds::new(p, Arc::clone(store), Arc::clone(clock), KeepOptions::default());
            let kept = Arc::new(kept.with_history(Arc::clone(history)));
            kept.keep_fresh(Arc::new(Sleep)).map(|()| kept as Arc<dyn Platform>)
        })
        .collect()
}

/// The platforms' grouping rules, before any name: Steam's depots under the
/// nearest app id below them. Battle.net's names come from its catalog, a
/// source of their own ([`BattleNet`]'s names).
pub(super) fn builtin_names() -> NameTable {
    let mut t = NameTable::default();
    t.group_numbered(STEAM_ID, DEPOT_WINDOW);
    t
}

/// Game names: the names file (if configured) over the grouping rules. Later
/// sources (a Steam login) go first in the chain.
pub(super) fn game_names(c: &Config) -> Result<Arc<dyn GameNames>, String> {
    let table = match &c.names {
        Some(p) => TableNames::open(builtin_names(), p)?,
        None => TableNames::new(builtin_names()),
    };
    println!("game names: {} known", table.len());
    Ok(Arc::new(ChainNames(vec![Arc::new(table)])))
}

/// The caching rules of every platform; other services are cached by the generic rules.
pub(super) fn profiles() -> Profiles {
    Profiles::new(vec![Arc::new(Steam), Arc::new(Blizzard), Arc::new(Epic), Arc::new(Riot)])
}

/// Battle.net's use cases. Plans and prefill ask the same cache clients do,
/// so what they fetch is stored as theirs is.
pub(super) fn battle_net(
    c: &Config,
    cache: &Arc<Cache>,
    store: &Arc<Store>,
    up: &Arc<dyn Upstream>,
    clock: &Arc<dyn Clock>,
    deleter: &Arc<GameDeleter>,
) -> BattleNet {
    let catalog = Arc::new(BlizzardCatalog::new(Arc::clone(up), Arc::clone(clock), CatalogOptions::default()));
    let region = c.blizzard_region.clone();
    // Every Battle.net game there is: the catalog's products, their CDN directories and versions.
    let products = Arc::new(BlizzardProducts::new(
        Arc::clone(&catalog) as Arc<dyn ProductList>,
        Arc::clone(up),
        Arc::clone(clock),
        ProductOptions { region: region.clone(), ..ProductOptions::default() },
    ));
    // The catalog finds a CDN path's product (its name and art) by the code the patch service names.
    let codes: Arc<dyn CodeOf> = Arc::clone(&products) as Arc<dyn CodeOf>;
    catalog.set_codes(Arc::downgrade(&codes));
    let client_path: Arc<dyn RequestHandler> = Arc::clone(cache) as Arc<dyn RequestHandler>;
    let mut tags = c.blizzard_tags.clone();
    tags.push(region.to_ascii_uppercase());
    let plans = Arc::new(BlizzardPlans::new(
        Arc::clone(&client_path),
        Arc::clone(store),
        Arc::clone(up),
        Arc::clone(clock),
        // Room for every stashed build's plan and, for the games looked at, another platform's.
        PlanOptions { region: region.clone(), tags, keep: 48, ..PlanOptions::default() },
    ));
    let builds = Arc::new(
        BlizzardBuilds::new(
            Arc::clone(up),
            Arc::clone(store),
            Arc::clone(clock),
            BuildOptions { region, ..BuildOptions::default() },
        )
        .with_plans(plans)
        .with_products(Arc::clone(&products)),
    );
    // Prune: plans of every platform and language (no tags), so nothing any client needs goes.
    let full_plans = Arc::new(BlizzardPlans::new(
        Arc::clone(&client_path),
        Arc::clone(store),
        Arc::clone(up),
        Arc::clone(clock),
        PlanOptions { region: c.blizzard_region.clone(), tags: Vec::new(), keep: 4, ..PlanOptions::default() },
    ));
    let pruner =
        Arc::new(Pruner::new(Arc::clone(&builds), full_plans, Arc::clone(store), Arc::clone(clock), PRUNE_GRACE_MS));
    let jobs = Arc::new(BlizzardJobs::new(Arc::clone(&builds)));
    let prefill_opts = PrefillOptions { parallel: c.prefill_parallel, ..PrefillOptions::default() };
    // What the CDN sends is checked against the hashes in the files' names before it is stored.
    let mut prefill = Prefiller::new(jobs, BATTLENET, client_path, Arc::clone(store), Arc::clone(clock), prefill_opts)
        .with_pause(Arc::new(Sleep))
        .with_checked_cdn(Arc::clone(up), stash_put(c, store, BATTLENET));
    let mut platform = BattleNet::new(catalog).with_products(products).with_pruner(Arc::clone(&pruner));
    // lancache's old cache, read-only: what it holds of a game's current build, and a prefill source.
    if let Some(dir) = &c.nginx_cache {
        let source: Arc<dyn NginxCacheSource> = Arc::new(NginxDir::new(dir));
        let probe = NginxProbe::new(Arc::clone(&builds), Arc::clone(&source), ProbeOptions::default());
        platform = platform.with_nginx(Arc::new(probe));
        prefill = prefill.with_lancache(lancache_read(source, BATTLENET), stash_put(c, store, BATTLENET));
    }
    let prefill = Arc::new(prefill);
    // Old builds go on their own: hourly, the first game whose live build is stashed whole and
    // has files no current build needs, unused for the grace period.
    let (p, b, d, jobs) = (Arc::clone(&pruner), Arc::clone(&builds), Arc::clone(deleter), Arc::clone(&prefill));
    let spawned = std::thread::Builder::new().name("lanstash-prune".into()).spawn(move || {
        loop {
            std::thread::sleep(AUTO_PRUNE_EVERY);
            let busy = |game: &str| jobs.status().is_some_and(|s| s.game == game && s.state == PrefillState::Running);
            if let Some((game, bytes)) = prune_once(&p, &b, &d, &busy) {
                println!("pruning {game}: {:.2} GB no current build needs", bytes as f64 / 1e9);
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("automatic pruning not started: {e}");
    }
    platform.with_prefill(prefill).with_builds(builds)
}

/// Old builds' files stay a week after their last use (a client still on them, a rollback).
pub(super) const PRUNE_GRACE_MS: u64 = 7 * 86_400_000;

/// Time between automatic prune passes.
pub(super) const AUTO_PRUNE_EVERY: std::time::Duration = std::time::Duration::from_secs(3_600);

/// Reads `service`'s pieces out of lancache's cache, keyed as lancache keys them.
pub(super) fn lancache_read(source: Arc<dyn NginxCacheSource>, service: &str) -> LancacheRead {
    let opts = LancacheOptions { service: service.into(), slice_bytes: 1 << 20, max_piece: 512 << 20 };
    LancacheRead::new(source, opts)
}

/// Puts `service`'s pieces from elsewhere where a download would.
pub(super) fn stash_put(c: &Config, store: &Arc<Store>, service: &str) -> StashPut {
    StashPut::new(Arc::clone(store), profiles().get(service), c.object_file_min)
}
