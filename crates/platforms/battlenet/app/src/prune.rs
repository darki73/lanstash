//! Use case: what of a Battle.net game's directory no build players get any
//! more needs: object files (archives and large loose files) that no current
//! or pre-release build of any product installed from the directory (World of
//! Warcraft's retail, Classic, Forever, ...) uses, for any platform or language
//! (its plan with no tags chosen: every file of the build). Files used within
//! the grace period stay (a client still on the old build, a rollback). If any
//! of those builds cannot be looked up or planned, nothing is prunable: an
//! unknown build might need anything.
//!
//! [`prune_once`] is the automatic pass: games whose live build is stashed
//! whole get pruned (the composition root runs it hourly).
//!
//! Segments (small whole objects, a few per cent of the bytes) are not pruned:
//! they mix objects of several builds.

use std::collections::BTreeSet;
use std::sync::Arc;

use lanstash_battlenet_domain::tact::Version;
use lanstash_domain::object::object_key;
use lanstash_ports::clock::Clock;
use lanstash_store::evict::EvictItem;
use lanstash_store::layout::GameDir;
use lanstash_store::store::Store;

use crate::builds::BlizzardBuilds;
use crate::plans::BlizzardPlans;

/// What a prune would delete, and what it keeps for now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prunable {
    /// Object files (journal numbers) no current build needs, unused for the grace period.
    pub files: Vec<u32>,
    pub bytes: u64,
    /// Bytes no current build needs but used within the grace period.
    pub recent_bytes: u64,
}

pub struct Pruner {
    builds: Arc<BlizzardBuilds>,
    /// Plans of every file of a build (no tags chosen).
    full: Arc<BlizzardPlans>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    grace_ms: u64,
}

impl std::fmt::Debug for Pruner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pruner").field("grace_ms", &self.grace_ms).finish_non_exhaustive()
    }
}

impl Pruner {
    /// `full`: plans made with no tags (every platform and language).
    #[must_use]
    pub fn new(
        builds: Arc<BlizzardBuilds>,
        full: Arc<BlizzardPlans>,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        grace_ms: u64,
    ) -> Self {
        Self { builds, full, store, clock, grace_ms }
    }

    /// What of `game`'s directory a prune would delete now.
    ///
    /// # Errors
    /// A build of the directory could not be looked up or planned.
    pub fn find(&self, game: &str) -> Result<Prunable, String> {
        let c = self.builds.check(game);
        let product =
            c.product.clone().ok_or_else(|| c.error.clone().unwrap_or_else(|| format!("{game}: no current build")))?;
        let mut builds: Vec<(String, &Version)> = Vec::new();
        for b in [&c.build, &c.prerelease].into_iter().flatten() {
            builds.push((product.clone(), &b.version));
        }
        for f in &c.flavours {
            for b in [&f.build, &f.prerelease].into_iter().flatten() {
                builds.push((f.product.clone(), &b.version));
            }
        }
        let mut needed: BTreeSet<String> = BTreeSet::new();
        for (product, version) in builds {
            let plan = self.full.plan(&product, version)?;
            needed.extend(plan.files.iter().map(|(t, _)| object_key("blizzard", t)));
            let used: BTreeSet<usize> = plan.parts.iter().map(|p| p.0).collect();
            needed.extend(used.into_iter().filter_map(|a| plan.archives.get(a)).map(|t| object_key("blizzard", t)));
        }
        let dir = GameDir::new("blizzard", game);
        let keys = self.store.object_files_of(&dir);
        let now = self.clock.now_ms();
        let mut out = Prunable::default();
        for v in self.store.eviction_candidates().into_iter().filter(|v| v.game == dir) {
            let EvictItem::File(no) = v.item else { continue };
            let Some((_, key)) = keys.iter().find(|(n, _)| *n == no) else { continue };
            if needed.contains(key) {
                continue;
            }
            if now.saturating_sub(v.last_used_ms) < self.grace_ms {
                out.recent_bytes += v.bytes;
            } else {
                out.files.push(no);
                out.bytes += v.bytes;
            }
        }
        Ok(out)
    }
}

/// One pass of automatic pruning: the first game whose live build is stashed
/// whole (a pre-release does not count: its files are kept, not a reason to
/// prune) and has something prunable, unless a deletion runs or `busy` says
/// the game is being prefilled. Starts at most one deletion job; returns the
/// game and the bytes it deletes.
pub fn prune_once(
    pruner: &Pruner,
    builds: &BlizzardBuilds,
    deleter: &lanstash_app::delete_game::GameDeleter,
    busy: &dyn Fn(&str) -> bool,
) -> Option<(String, u64)> {
    use lanstash_app::delete_game::DeleteState;
    if deleter.status().is_some_and(|s| s.state == DeleteState::Running) {
        return None;
    }
    for c in builds.check_all() {
        if !c.current() || busy(&c.game) {
            continue;
        }
        let Ok(p) = pruner.find(&c.game) else { continue };
        if p.files.is_empty() {
            continue;
        }
        let bytes = p.bytes;
        if deleter.start_files("blizzard", &c.game, p.files).is_ok() {
            return Some((c.game, bytes));
        }
        return None;
    }
    None
}
