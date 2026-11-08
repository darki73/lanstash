//! The stashed Epic games' builds ([`EpicBuilds`]): each app the stash holds
//! some of (its download directories grouped by app), its current build's
//! chunks (from the manifest, read once per build), and how many of them the
//! stash holds whole. It renders the builds view (`GET /api/v1/builds/
//! epicgames`, Steam's shape: a game is current when the stash holds every
//! chunk) and says where each build stands, for automatic updates. It looks
//! every half minute and refreshes when Epic's part of the stash or a build
//! changed (at least every ten minutes); a new build's manifest is read then.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_app::look::until_changed;
use lanstash_app::once_per::OncePer;
use lanstash_domain::extent::Span;
use lanstash_domain::json::string;
use lanstash_ports::clock::Pause;
use lanstash_ports::names::GameNames;
use lanstash_ports::platform::BuildStatus;
use lanstash_store::index::Lookup;
use lanstash_store::store::Store;

use crate::library::EpicLibrary;
use crate::prefill::EpicPlanner;

/// How often the thread looks whether anything a refresh depends on moved.
const LOOK_MS: u64 = 30 * 1000;
/// The longest between two refreshes all the same.
const EVERY_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone)]
struct Entry {
    version: String,
    /// The build's chunks: object keys and sizes.
    chunks: Option<Arc<Vec<(String, u64)>>>,
    error: Option<String>,
}

/// A stashed app, its build, and the bytes of it held and wanted (once known).
type Row = (String, Entry, Option<(u64, u64)>);

/// The stashed Epic games' builds.
pub struct EpicBuilds {
    library: Arc<EpicLibrary>,
    planner: Arc<EpicPlanner>,
    store: Arc<Store>,
    entries: Mutex<BTreeMap<String, Entry>>,
    /// Moves each time `entries` changes ([`EpicBuilds::mark`]).
    changes: AtomicU64,
    /// The rows as last worked out, by what they depend on ([`EpicBuilds::rows`]).
    worked: OncePer<(u64, u64), Vec<Row>>,
}

impl std::fmt::Debug for EpicBuilds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicBuilds").finish_non_exhaustive()
    }
}

impl EpicBuilds {
    #[must_use]
    pub fn new(library: Arc<EpicLibrary>, planner: Arc<EpicPlanner>, store: Arc<Store>) -> Self {
        Self {
            library,
            planner,
            store,
            entries: Mutex::default(),
            changes: AtomicU64::new(0),
            worked: OncePer::default(),
        }
    }

    /// Moves each time the builds change their own idea (a game's build read, a game forgotten).
    #[must_use]
    pub fn builds_mark(&self) -> u64 {
        self.changes.load(Ordering::Relaxed)
    }

    /// The apps the stash holds some of.
    fn apps(&self) -> Vec<String> {
        let mut apps: Vec<String> = Vec::new();
        for g in self.store.usage().keys().filter(|g| g.service == "epicgames") {
            if let Some((_, a)) = self.library.app(&g.game)
                && !apps.contains(&a.app_name)
            {
                apps.push(a.app_name);
            }
        }
        apps
    }

    /// Reads the current build of each stashed app whose build is new, and forgets the apps no
    /// longer stashed.
    pub fn refresh(&self) {
        let apps = self.apps();
        // A game no longer stashed (deleted, evicted) leaves the view, as Steam's do.
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            let before = entries.len();
            entries.retain(|app, _| apps.contains(app));
            if entries.len() != before {
                self.changes.fetch_add(1, Ordering::Relaxed);
            }
        }
        for app in apps {
            let Some((_, asset)) = self.library.app(&app) else { continue };
            let known = self.entries.lock().unwrap_or_else(PoisonError::into_inner).get(&app).cloned();
            if known.is_some_and(|e| e.version == asset.build_version && e.chunks.is_some()) {
                continue;
            }
            let entry = match self.planner.current(&app) {
                Ok(c) => Entry { chunks: Some(Arc::new(c.chunk_keys())), version: c.version, error: None },
                Err(e) => Entry { version: asset.build_version, chunks: None, error: Some(e) },
            };
            self.entries.lock().unwrap_or_else(PoisonError::into_inner).insert(app, entry);
            self.changes.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Bytes of `chunks` the stash holds whole.
    fn held(&self, chunks: &[(String, u64)]) -> u64 {
        chunks
            .iter()
            .filter(|(key, _)| {
                matches!(self.store.lookup_quietly(key), Lookup::Found(o) if o.total > 0 && o.covers(Span::new(0, o.total)))
            })
            .map(|(_, size)| size)
            .sum()
    }

    /// Each stashed app: its build, the bytes held and wanted, why not known. Worked out again only
    /// when Epic's part of the stash or the builds moved: the view and the status, asked one after
    /// the other, share one walk over the chunks.
    fn rows(&self) -> Arc<Vec<Row>> {
        let key = (self.store.changes("epicgames"), self.changes.load(Ordering::Relaxed));
        self.worked.get(key, || {
            let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner).clone();
            entries
                .into_iter()
                .map(|(app, e)| {
                    let figures = e.chunks.as_ref().map(|c| (self.held(c), c.iter().map(|(_, s)| s).sum()));
                    (app, e, figures)
                })
                .collect()
        })
    }

    /// The builds view.
    #[must_use]
    pub fn view(&self) -> String {
        let num = |n: Option<u64>| n.map_or_else(|| "null".to_string(), |n| n.to_string());
        let games: Vec<String> = self
            .rows()
            .iter()
            .map(|(app, e, f)| {
                let f = *f;
                let name = self.library.name("epicgames", app).map_or_else(|| "null".to_string(), |n| string(&n));
                let error = e.error.as_deref().map_or_else(|| "null".to_string(), string);
                let current = f.is_some_and(|(have, want)| have == want);
                format!(
                    "{{\"game\":{},\"product\":null,\"name\":{name},\"version\":{},\"build_config\":null,\"have\":{},\"want\":{},\"plan_error\":{error},\"current\":{current},\"prerelease\":null,\"error\":null,\"lancache_share\":null}}",
                    string(app),
                    string(&e.version),
                    num(f.map(|x| x.0)),
                    num(f.map(|x| x.1)),
                )
            })
            .collect();
        format!("{{\"region\":\"windows\",\"lancache\":false,\"games\":[{}]}}\n", games.join(","))
    }

    /// The stashed apps whose current build the stash holds whole.
    #[must_use]
    pub fn current(&self) -> Vec<String> {
        self.status().into_iter().filter(|s| s.current).map(|s| s.game).collect()
    }

    /// Where each stashed app's current build stands (for automatic updates).
    #[must_use]
    pub fn status(&self) -> Vec<BuildStatus> {
        self.rows()
            .iter()
            .map(|(app, e, f)| BuildStatus {
                game: app.clone(),
                version: e.version.clone(),
                known: f.is_some(),
                current: f.is_some_and(|(have, want)| have == want),
            })
            .collect()
    }

    /// Keeps the builds fresh on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("epic-builds", Arc::clone(&pause), move || {
            let before = me.mark();
            me.refresh();
            until_changed(pause.as_ref(), LOOK_MS, EVERY_MS, &before, &|| me.mark());
            lanstash_app::rounds::Next::Again
        })
    }

    /// What a refresh depends on, cheap to look at: how often Epic's part of the stash changed,
    /// and the library's builds. Finding the stashed apps walks the whole index; it is done
    /// only when this moves (or every [`EVERY_MS`], for a manifest that could not be read).
    fn mark(&self) -> (u64, Vec<(String, String)>) {
        let builds = self.library.games().into_iter().map(|a| (a.app_name, a.build_version)).collect();
        (self.store.changes("epicgames"), builds)
    }
}
