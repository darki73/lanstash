//! Steam as the core sees it: an id, its cover art, and what the composition
//! root gives it once Steam's use cases run (`lanstash-steam-app`): its
//! account (the QR sign-in), its prefill, its builds and catalog views, and its names
//! (the account's library: each depot named after its app, exactly).

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use lanstash_ports::account::Account;
use lanstash_ports::art::ArtSource;
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::{BuildStatus, BuildsMarkOf, Platform, Pruning, View, ViewError};
use lanstash_ports::prefill::Prefill;

use crate::art::SteamArt;

/// Draws a view (the builds, the catalog).
pub type BuildsView = Arc<dyn Fn() -> View + Send + Sync>;

/// Steam's service identifier.
pub const ID: &str = "steam";

/// The Steam platform.
#[derive(Clone, Default)]
pub struct SteamPlatform {
    account: Option<Arc<dyn Account>>,
    prefill: Option<Arc<dyn Prefill>>,
    names: Option<Arc<dyn GameNames>>,
    builds: Option<BuildsView>,
    catalog: Option<BuildsView>,
    parts: Option<PartsOf>,
    forget: Option<ForgetOf>,
    prune: Option<PruneOf>,
    nginx: Option<NginxOf>,
    status: Option<StatusOf>,
    mark: Option<BuildsMarkOf>,
    art: SteamArt,
}

/// What of a game no current build needs.
pub type PruneOf = Arc<dyn Fn(&str) -> Result<Pruning, ViewError> + Send + Sync>;

/// Where each stashed game's current build stands.
pub type StatusOf = Arc<dyn Fn() -> Vec<BuildStatus> + Send + Sync>;

/// Why lancache's old cache gives a game's chunks or not (a diagnostic view).
pub type NginxOf = Arc<dyn Fn(&str) -> View + Send + Sync>;

/// The store directories of a game (an app's depots).
pub type PartsOf = Arc<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>;

/// Drops what is kept of a game being deleted.
pub type ForgetOf = Arc<dyn Fn(&str) + Send + Sync>;

impl core::fmt::Debug for SteamPlatform {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SteamPlatform")
            .field("account", &self.account.is_some())
            .field("prefill", &self.prefill.is_some())
            .field("names", &self.names.is_some())
            .field("builds", &self.builds.is_some())
            .field("catalog", &self.catalog.is_some())
            .field("parts", &self.parts.is_some())
            .field("forget", &self.forget.is_some())
            .field("prune", &self.prune.is_some())
            .field("nginx", &self.nginx.is_some())
            .field("status", &self.status.is_some())
            .finish()
    }
}

impl SteamPlatform {
    /// With its account.
    #[must_use]
    pub fn with_account(mut self, account: Arc<dyn Account>) -> Self {
        self.account = Some(account);
        self
    }

    /// With its prefill jobs.
    #[must_use]
    pub fn with_prefill(mut self, prefill: Arc<dyn Prefill>) -> Self {
        self.prefill = Some(prefill);
        self
    }

    /// With its builds view.
    #[must_use]
    pub fn with_builds(mut self, builds: BuildsView) -> Self {
        self.builds = Some(builds);
        self
    }

    /// With its catalog view (the account's games).
    #[must_use]
    pub fn with_catalog(mut self, catalog: BuildsView) -> Self {
        self.catalog = Some(catalog);
        self
    }

    /// With the store directories each game is made of.
    #[must_use]
    pub fn with_parts(mut self, parts: PartsOf) -> Self {
        self.parts = Some(parts);
        self
    }

    /// With what drops a deleted game's kept state.
    #[must_use]
    pub fn with_forget(mut self, forget: ForgetOf) -> Self {
        self.forget = Some(forget);
        self
    }

    /// With the prune of old builds.
    #[must_use]
    pub fn with_prune(mut self, prune: PruneOf) -> Self {
        self.prune = Some(prune);
        self
    }

    /// With its art (by the file names its library's app info gives).
    #[must_use]
    pub fn with_art(mut self, art: SteamArt) -> Self {
        self.art = art;
        self
    }

    /// With where its stashed games' builds stand.
    #[must_use]
    pub fn with_status(mut self, status: StatusOf) -> Self {
        self.status = Some(status);
        self
    }

    /// Says when the builds change their own idea ([`Platform::builds_mark`]).
    #[must_use]
    pub fn with_builds_mark(mut self, mark: BuildsMarkOf) -> Self {
        self.mark = Some(mark);
        self
    }

    /// With the lancache inspection of a game.
    #[must_use]
    pub fn with_nginx(mut self, nginx: NginxOf) -> Self {
        self.nginx = Some(nginx);
        self
    }

    /// With its names.
    #[must_use]
    pub fn with_names(mut self, names: Arc<dyn GameNames>) -> Self {
        self.names = Some(names);
        self
    }
}

impl Platform for SteamPlatform {
    fn id(&self) -> &str {
        ID
    }

    fn names(&self) -> Option<Arc<dyn GameNames>> {
        self.names.clone()
    }

    fn art(&self) -> Option<Arc<dyn ArtSource>> {
        Some(Arc::new(self.art.clone()))
    }

    fn builds(&self) -> Option<View> {
        self.builds.as_ref().map(|b| b())
    }

    fn catalog(&self) -> Option<View> {
        self.catalog.as_ref().map(|c| c())
    }

    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        self.prefill.clone()
    }

    fn prune(&self, game: &str) -> Option<Result<Pruning, ViewError>> {
        self.prune.as_ref().map(|p| p(game))
    }

    fn parts(&self, game: &str) -> Option<Vec<String>> {
        self.parts.as_ref().and_then(|p| p(game))
    }

    fn forget(&self, game: &str) {
        if let Some(f) = &self.forget {
            f(game);
        }
    }

    fn account(&self) -> Option<Arc<dyn Account>> {
        self.account.clone()
    }

    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        self.status.as_ref().map(|s| s())
    }

    fn builds_mark(&self) -> u64 {
        self.mark.as_ref().map_or(0, |m| m())
    }

    fn nginx(&self, game: &str) -> Option<View> {
        self.nginx.as_ref().map(|n| n(game))
    }
}
