//! What of an Epic game no current build needs ([`EpicPruner`]): its download
//! directories' segments that hold none of the current build's chunks (the
//! segment rules: [`lanstash_app::prune_segments`]). Only the directories the
//! library files under the game are looked at; nothing is prunable when its
//! manifest cannot be read.

use std::collections::BTreeSet;
use std::sync::Arc;

use lanstash_app::prune_segments::{Prunable, prunable};
use lanstash_ports::clock::Clock;
use lanstash_store::store::Store;

use crate::library::EpicLibrary;
use crate::prefill::EpicPlanner;

/// Finds what of an Epic game no current build needs.
pub struct EpicPruner {
    library: Arc<EpicLibrary>,
    planner: Arc<EpicPlanner>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    grace_ms: u64,
}

impl std::fmt::Debug for EpicPruner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicPruner").field("grace_ms", &self.grace_ms).finish_non_exhaustive()
    }
}

impl EpicPruner {
    #[must_use]
    pub fn new(
        library: Arc<EpicLibrary>,
        planner: Arc<EpicPlanner>,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        grace_ms: u64,
    ) -> Self {
        Self { library, planner, store, clock, grace_ms }
    }

    /// What of `game` (an app, or one of its download directories) no current build needs.
    ///
    /// # Errors
    /// Not in the library, or its current manifest could not be read.
    pub fn find(&self, game: &str) -> Result<Prunable, String> {
        let (_, asset) = self.library.app(game).ok_or(format!("{game} is not in the Epic library"))?;
        let current = self.planner.current(&asset.app_name)?;
        let keys: Vec<String> = current.chunk_keys().into_iter().map(|(k, _)| k).collect();
        let dirs = self.dirs(&asset.app_name);
        Ok(prunable(&self.store, "epicgames", &dirs, &keys, self.clock.now_ms(), self.grace_ms))
    }

    /// The store directories of `game` (an app, or one of its download directories): those
    /// the library files under its app.
    #[must_use]
    pub fn dirs(&self, game: &str) -> BTreeSet<String> {
        let Some((_, asset)) = self.library.app(game) else { return BTreeSet::new() };
        self.store
            .usage()
            .into_keys()
            .filter(|g| g.service == "epicgames")
            .filter(|g| self.library.app(&g.game).is_some_and(|(_, a)| a.app_name == asset.app_name))
            .map(|g| g.game)
            .collect()
    }
}
