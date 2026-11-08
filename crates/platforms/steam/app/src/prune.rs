//! What of a Steam app no current build needs. Steam's chunks are small, so
//! they sit in segments (many chunks each); a segment goes only when none of
//! its chunks is needed: no current manifest of any of the app's depots (any
//! operating system, language or DLC) lists one, and it was not used within
//! the grace period. Old builds' chunks were written together, so their
//! segments go whole. A segment mixing old and current chunks is compacted
//! (its current chunks copied out, then it goes) once at most half of it is
//! current chunks and it was not used within the grace period; otherwise it
//! stays. Nothing is prunable when a manifest cannot be read. Only the depots
//! the account's licenses grant are looked at (Steam gives only their
//! manifests); the directories of other depots are never touched.

use std::collections::BTreeSet;
use std::sync::Arc;

use lanstash_app::prune_segments::{Prunable, prunable};
use lanstash_domain::object::object_key;
use lanstash_ports::clock::Clock;
use lanstash_steam_domain::manifest::chunk_path;
use lanstash_store::store::Store;

use crate::library::SteamLibrary;
use crate::manifests::{self, Servers};
use crate::session::Services;

/// A manifest: the app whose it is, the depot, the manifest id.
type ManifestRef = (u32, u32, u64);

/// Finds what of an app no current build needs.
pub struct SteamPruner {
    services: Services,
    library: Arc<SteamLibrary>,
    store: Arc<Store>,
    clock: Arc<dyn Clock>,
    grace_ms: u64,
}

impl std::fmt::Debug for SteamPruner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamPruner").field("grace_ms", &self.grace_ms).finish_non_exhaustive()
    }
}

impl SteamPruner {
    #[must_use]
    pub fn new(
        services: Services,
        library: Arc<SteamLibrary>,
        store: Arc<Store>,
        clock: Arc<dyn Clock>,
        grace_ms: u64,
    ) -> Self {
        Self { services, library, store, clock, grace_ms }
    }

    /// What of app `game` (an app id) no current build needs.
    ///
    /// # Errors
    /// No library yet, not an app of it, or a manifest that could not be read.
    pub fn find(&self, game: &str) -> Result<Prunable, String> {
        let lib = self.library.get().ok_or("no Steam library yet")?;
        let id: u32 = game.parse().map_err(|_| format!("{game} is not a Steam app id"))?;
        let app = lib.app(id).ok_or(format!("app {id} is not in the account's library"))?;
        // Every chunk of every current manifest of the depots the account owns of the
        // app and of its DLCs that have depots of their own (fetched as the DLC's),
        // whoever they are for; only their directories are pruned.
        let dlcs = lib.apps.iter().filter(|x| x.is_dlc_of(app) && lib.game_of(x).is_some_and(|g| g.id == id));
        let mut seen = BTreeSet::new();
        let depots: Vec<(u32, u32, u64)> = std::iter::once(app)
            .chain(dlcs)
            .flat_map(|owner| owner.depots.iter().map(move |d| (owner.id, d)))
            .filter(|(_, d)| d.from_app.is_none() && lib.grants(d.id) && seen.insert(d.id))
            .filter_map(|(owner, d)| Some((owner, d.id, d.manifest?)))
            .collect();
        let dirs: BTreeSet<String> = depots.iter().map(|(_, d, _)| d.to_string()).collect();
        // Each depot's manifest as an account whose licenses grant it.
        let accounts = self.services.accounts();
        let mut groups: Vec<(String, Vec<ManifestRef>)> = Vec::new();
        for item in &depots {
            let Some(account) = lib.account_for(item.0, item.1, &accounts) else { continue };
            match groups.iter_mut().find(|(a, _)| *a == account) {
                Some((_, g)) => g.push(*item),
                None => groups.push((account, vec![*item])),
            }
        }
        let mut servers: Option<Servers> = None;
        let mut keys = Vec::new();
        for (account, group) in &groups {
            keys.extend(self.services.with_session_as(account, |conn, on| {
                let servers = servers.get_or_insert_with(|| Servers::new(on.cell_id));
                let mut keys = Vec::new();
                for (owner, depot, gid) in group {
                    let (up, lists) = (self.services.upstream.as_ref(), &self.services.lists);
                    let chunks = manifests::chunks(conn, up, lists, servers, (*owner, *depot, *gid), true)?;
                    keys.extend(chunks.iter().map(|c| object_key("steam", &chunk_path(*depot, c))));
                }
                Ok(keys)
            })?);
        }
        Ok(prunable(&self.store, "steam", &dirs, &keys, self.clock.now_ms(), self.grace_ms))
    }
}

/// One step of the automatic prune ([`lanstash_app::prune_segments::prune_once`]) over the apps
/// whose current build is stashed whole.
pub fn prune_once(
    pruner: &SteamPruner,
    builds: &crate::builds::SteamBuilds,
    deleter: &lanstash_app::delete_game::GameDeleter,
    busy: &dyn Fn(&str) -> bool,
) -> Option<(String, u64)> {
    let current: Vec<String> = builds.current().iter().map(u32::to_string).collect();
    lanstash_app::prune_segments::prune_once("steam", &current, &|g| pruner.find(g), deleter, busy)
}
