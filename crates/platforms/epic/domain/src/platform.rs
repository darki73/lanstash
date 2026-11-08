//! Epic as the core sees it: an id, the file that sets up a Windows PC so
//! that lanstash keeps its launcher's downloads ([`crate::setup`]), and, when
//! the composition root gives them, the accounts lanstash signs in to, the
//! games' names and images, the catalog view of the accounts' games, their
//! builds, prefill, prune, and the store directories each game is made of.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use lanstash_ports::account::Account;
use lanstash_ports::art::ArtSource;
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::{BuildStatus, BuildsMarkOf, ClientOs, Platform, Pruning, SetupFile, View, ViewError};
use lanstash_ports::prefill::Prefill;

use crate::setup::{FILE_NAME, windows_cmd};

/// The catalog view, rendered when asked.
pub type CatalogView = Arc<dyn Fn() -> View + Send + Sync>;

/// The Epic Games platform.
#[derive(Default)]
pub struct EpicPlatform {
    account: Option<Arc<dyn Account>>,
    names: Option<Arc<dyn GameNames>>,
    art: Option<Arc<dyn ArtSource>>,
    catalog: Option<CatalogView>,
    prefill: Option<Arc<dyn Prefill>>,
    builds: Option<CatalogView>,
    status: Option<StatusOf>,
    mark: Option<BuildsMarkOf>,
    prune: Option<PruneOf>,
    parts: Option<PartsOf>,
}

/// Where each stashed game's current build stands, said when asked.
pub type StatusOf = Arc<dyn Fn() -> Vec<BuildStatus> + Send + Sync>;

/// What of a game no current build needs: a view of it, and the items a prune deletes.
pub type PruneOf = Arc<dyn Fn(&str) -> Result<Pruning, ViewError> + Send + Sync>;

/// The store directories a game is made of (Epic files an app's downloads under another id).
pub type PartsOf = Arc<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>;

impl core::fmt::Debug for EpicPlatform {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EpicPlatform")
            .field("account", &self.account.is_some())
            .field("catalog", &self.catalog.is_some())
            .finish_non_exhaustive()
    }
}

impl EpicPlatform {
    /// Offers `account`: the Epic accounts lanstash signs in to.
    #[must_use]
    pub fn with_account(mut self, account: Arc<dyn Account>) -> Self {
        self.account = Some(account);
        self
    }

    /// Names the games from `names` (the accounts' library).
    #[must_use]
    pub fn with_names(mut self, names: Arc<dyn GameNames>) -> Self {
        self.names = Some(names);
        self
    }

    /// Finds the games' images with `art`.
    #[must_use]
    pub fn with_art(mut self, art: Arc<dyn ArtSource>) -> Self {
        self.art = Some(art);
        self
    }

    /// Offers the stashed games' builds view, and where each stands (for automatic updates).
    #[must_use]
    pub fn with_builds(mut self, view: CatalogView, status: StatusOf) -> Self {
        self.builds = Some(view);
        self.status = Some(status);
        self
    }

    /// Says when the builds change their own idea ([`Platform::builds_mark`]).
    #[must_use]
    pub fn with_builds_mark(mut self, mark: BuildsMarkOf) -> Self {
        self.mark = Some(mark);
        self
    }

    /// Prefills the accounts' games with `prefill`.
    #[must_use]
    pub fn with_prefill(mut self, prefill: Arc<dyn Prefill>) -> Self {
        self.prefill = Some(prefill);
        self
    }

    /// Offers the accounts' games as the catalog view.
    #[must_use]
    pub fn with_catalog(mut self, catalog: CatalogView) -> Self {
        self.catalog = Some(catalog);
        self
    }

    /// Enables prune (what of a game no current build needs).
    #[must_use]
    pub fn with_prune(mut self, prune: PruneOf) -> Self {
        self.prune = Some(prune);
        self
    }

    /// Says which store directories a game is made of (deleting it deletes them all).
    #[must_use]
    pub fn with_parts(mut self, parts: PartsOf) -> Self {
        self.parts = Some(parts);
        self
    }
}

impl Platform for EpicPlatform {
    fn id(&self) -> &str {
        "epicgames"
    }

    fn names(&self) -> Option<Arc<dyn GameNames>> {
        self.names.clone()
    }

    fn art(&self) -> Option<Arc<dyn ArtSource>> {
        self.art.clone()
    }

    fn catalog(&self) -> Option<View> {
        self.catalog.as_ref().map(|c| c())
    }

    fn account(&self) -> Option<Arc<dyn Account>> {
        self.account.clone()
    }

    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        self.prefill.clone()
    }

    fn builds(&self) -> Option<View> {
        self.builds.as_ref().map(|b| b())
    }

    fn build_status(&self) -> Option<Vec<BuildStatus>> {
        self.status.as_ref().map(|s| s())
    }

    fn builds_mark(&self) -> u64 {
        self.mark.as_ref().map_or(0, |m| m())
    }

    fn prune(&self, game: &str) -> Option<Result<Pruning, ViewError>> {
        self.prune.as_ref().map(|p| p(game))
    }

    fn parts(&self, game: &str) -> Option<Vec<String>> {
        self.parts.as_ref().and_then(|p| p(game))
    }

    fn setup(&self, os: ClientOs) -> Option<SetupFile> {
        match os {
            ClientOs::Windows => Some(SetupFile {
                name: FILE_NAME.to_owned(),
                content_type: "application/octet-stream",
                body: windows_cmd().into_bytes(),
            }),
            ClientOs::MacOs | ClientOs::Linux => None,
        }
    }
}
