//! Battle.net as a [`Platform`]: the catalog for names and art, and the
//! builds, catalog, game, nginx and prefill views over the use cases the
//! composition root built.

use std::sync::Arc;

use lanstash_battlenet_app::builds::{BlizzardBuilds, BuildCheck, BuildState};
use lanstash_battlenet_app::catalog::BlizzardCatalog;
use lanstash_battlenet_app::nginx::NginxProbe;
use lanstash_battlenet_app::products::BlizzardProducts;
use lanstash_battlenet_app::prune::Pruner;
use lanstash_ports::art::ArtSource;
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::{BuildStatus, BuildsAndStatus, Platform, PruneItem, Pruning, View, ViewError};
use lanstash_ports::prefill::Prefill;

use crate::json;

/// Battle.net's service identifier.
pub const ID: &str = "blizzard";

/// Battle.net, with what was set up of it; each view is absent until its
/// use case is given.
#[derive(Debug)]
pub struct BattleNet {
    catalog: Arc<BlizzardCatalog>,
    products: Option<Arc<BlizzardProducts>>,
    builds: Option<Arc<BlizzardBuilds>>,
    nginx: Option<Arc<NginxProbe>>,
    pruner: Option<Arc<Pruner>>,
    prefill: Option<Arc<dyn Prefill>>,
}

impl BattleNet {
    /// The launcher catalog gives the names and art.
    #[must_use]
    pub fn new(catalog: Arc<BlizzardCatalog>) -> Self {
        Self { catalog, products: None, builds: None, nginx: None, pruner: None, prefill: None }
    }

    /// Enables the catalog view.
    #[must_use]
    pub fn with_products(mut self, products: Arc<BlizzardProducts>) -> Self {
        self.products = Some(products);
        self
    }

    /// Enables the builds and game views.
    #[must_use]
    pub fn with_builds(mut self, builds: Arc<BlizzardBuilds>) -> Self {
        self.builds = Some(builds);
        self
    }

    /// Enables prune (what of a game no current build needs).
    #[must_use]
    pub fn with_pruner(mut self, pruner: Arc<Pruner>) -> Self {
        self.pruner = Some(pruner);
        self
    }

    /// Enables the nginx view.
    #[must_use]
    pub fn with_nginx(mut self, probe: Arc<NginxProbe>) -> Self {
        self.nginx = Some(probe);
        self
    }

    /// Enables prefill.
    #[must_use]
    pub fn with_prefill(mut self, prefill: Arc<dyn Prefill>) -> Self {
        self.prefill = Some(prefill);
        self
    }

    /// Whether a prefill can take pieces from lancache's cache.
    fn lancache(&self) -> bool {
        self.prefill.as_ref().is_some_and(|p| p.lancache())
    }
}

/// Where each checked game's builds stand: the game's own product by its directory, the other
/// products of the directory by their codes.
fn status_of(checks: &[BuildCheck]) -> Vec<BuildStatus> {
    let status = |game: &str, b: &BuildState| BuildStatus {
        game: game.to_owned(),
        version: b.version.name.clone(),
        known: b.coverage.is_some(),
        current: b.complete(),
    };
    checks
        .iter()
        .flat_map(|c| {
            let own = c.build.as_ref().map(|b| status(&c.game, b));
            let others = c.flavours.iter().filter_map(|f| f.build.as_ref().map(|b| status(&f.product, b)));
            own.into_iter().chain(others).collect::<Vec<_>>()
        })
        .collect()
}

impl Platform for BattleNet {
    fn id(&self) -> &str {
        ID
    }

    fn names(&self) -> Option<Arc<dyn GameNames>> {
        Some(Arc::clone(&self.catalog) as Arc<dyn GameNames>)
    }

    fn art(&self) -> Option<Arc<dyn ArtSource>> {
        Some(Arc::clone(&self.catalog) as Arc<dyn ArtSource>)
    }

    fn builds(&self) -> Option<View> {
        let share = |game: &str, config: &str| self.nginx.as_ref().and_then(|p| p.share(game, config));
        self.builds.as_ref().map(|b| Ok(json::builds(b, self.lancache(), &share)))
    }

    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        self.builds.as_ref().map(|b| status_of(&b.check_all()))
    }

    fn builds_and_status(&self) -> BuildsAndStatus {
        // One check of every game (a walk over the stash and each plan) for both.
        let Some(b) = &self.builds else { return (None, None) };
        let checks = b.check_all();
        let share = |game: &str, config: &str| self.nginx.as_ref().and_then(|p| p.share(game, config));
        (Some(Ok(json::builds_of(&checks, b.region(), self.lancache(), &share))), Some(status_of(&checks)))
    }

    fn catalog(&self) -> Option<View> {
        self.products.as_ref().map(|p| p.list().map(|l| json::catalog(&l)).map_err(ViewError::Unavailable))
    }

    fn game(&self, game: &str) -> Option<View> {
        self.builds.as_ref().map(|b| Ok(json::game(b, game, self.lancache())))
    }

    fn prune(&self, game: &str) -> Option<Result<Pruning, ViewError>> {
        self.pruner.as_ref().map(|p| {
            p.find(game)
                .map(|r| {
                    let items = r.files.iter().map(|&no| PruneItem::File { dir: game.to_owned(), no }).collect();
                    (json::prune(game, &r), items)
                })
                .map_err(ViewError::Unavailable)
        })
    }

    fn nginx(&self, game: &str) -> Option<View> {
        self.nginx.as_ref().map(|p| p.probe(game).map(|r| json::probe(&r)).map_err(ViewError::Unavailable))
    }

    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        self.prefill.clone()
    }
}
