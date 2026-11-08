//! Which Steam apps the stash holds and how much of each one's current
//! build: the apps come from the stashed depots (the library says whose
//! they are), each app's install from its current manifests (read once per
//! build as chunk lists, kept between runs: after a restart only new builds'
//! manifests are downloaded, and an app whose lists are all kept needs no
//! Steam session), and what is held from the index, without counting as a
//! use. Kept fresh on a thread: within half a minute of Steam's part of the
//! stash or the library changing (a game just prefilled is worked out then, not
//! up to five minutes later), and every five minutes; the view is drawn on
//! demand. Once every app was worked out after the start,
//! [`SteamBuilds::ready`] is raised.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_app::look::until_changed;
use lanstash_app::once_per::OncePer;
use lanstash_domain::extent::Span;
use lanstash_domain::object::object_key;
use lanstash_ports::clock::Pause;
use lanstash_steam_domain::apps::{App, Target};
use lanstash_steam_domain::builds::{AppBuild, render};
use lanstash_steam_domain::manifest::{Chunk, chunk_path};
use lanstash_store::index::Lookup;
use lanstash_store::store::Store;

use crate::lancache::SteamLancache;
use crate::library::{Library, SteamLibrary};
use crate::manifests::{self, Servers};
use crate::session::Services;

/// The longest the stashed apps' builds go without being looked at again.
const LOOK_MS: u64 = 5 * 60 * 1000;
/// How often the thread looks whether Steam's part of the stash or the library moved.
const CHANGE_LOOK_MS: u64 = 30 * 1000;
/// How soon to look again while there is no library yet (just after a start or a sign-in):
/// looking costs nothing until there is one.
const SOON_MS: u64 = 2 * 1000;

/// One app's current build as last read.
#[derive(Debug, Clone)]
struct Entry {
    build: Option<u64>,
    /// Each unique chunk's object key and size; `None` when it could not be read.
    chunks: Option<Arc<Vec<(String, u64)>>>,
    error: Option<String>,
}

/// The stashed apps' builds.
pub struct SteamBuilds {
    services: Services,
    library: Arc<SteamLibrary>,
    store: Arc<Store>,
    target: Target,
    /// Whether a prefill can take chunks from lancache's old cache.
    lancache: bool,
    entries: Mutex<BTreeMap<u32, Entry>>,
    /// What lancache's old cache holds of each app (for those not stashed whole).
    probe: Option<Arc<SteamLancache>>,
    /// Raised once every stashed app was worked out (the first whole refresh): what else asks Steam waits for it.
    ready: Arc<AtomicBool>,
    /// Moves each time an app's install is worked out ([`SteamBuilds::mark`]).
    changes: AtomicU64,
    /// The apps' builds as last worked out, by what they depend on ([`SteamBuilds::apps`]).
    worked: OncePer<(u64, u64, u64), Vec<AppBuild>>,
}

impl std::fmt::Debug for SteamBuilds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamBuilds").field("target", &self.target).finish_non_exhaustive()
    }
}

/// Chunks of `depot` as object keys and sizes.
fn keyed(depot: u32, chunks: &[Chunk]) -> impl Iterator<Item = (String, u64)> + '_ {
    chunks.iter().map(move |c| (object_key("steam", &chunk_path(depot, c)), u64::from(c.compressed)))
}

/// An app's install as chunk keys and sizes from the kept chunk lists alone; none when one is not kept.
fn kept_install(services: &Services, (lib, app): (&Library, &App), t: &Target) -> Option<Vec<(String, u64)>> {
    let mut out = Vec::new();
    for (_, d) in lib.install(app, t) {
        out.extend(keyed(d.id, &services.lists.kept(d.id, d.manifest?)?));
    }
    Some(out)
}

/// An app's install as chunk keys and sizes, its manifests downloaded where their lists are not
/// kept: each depot's as an account whose licenses grant it, on that account's session.
fn install(
    services: &Services,
    servers: &mut Option<Servers>,
    (lib, app): (&Library, &App),
    t: &Target,
) -> Result<Vec<(String, u64)>, String> {
    let chunks = manifests::install_chunks(services, servers, lib, &lib.install(app, t), true)?;
    Ok(chunks.iter().map(|(depot, c)| (object_key("steam", &chunk_path(*depot, c)), u64::from(c.compressed))).collect())
}

impl SteamBuilds {
    #[must_use]
    pub fn new(
        services: Services,
        library: Arc<SteamLibrary>,
        store: Arc<Store>,
        target: Target,
        lancache: bool,
    ) -> Self {
        Self {
            services,
            library,
            store,
            target,
            lancache,
            entries: Mutex::new(BTreeMap::new()),
            probe: None,
            ready: Arc::default(),
            changes: AtomicU64::new(0),
            worked: OncePer::default(),
        }
    }

    /// Raised once every stashed app was worked out after the start.
    #[must_use]
    pub fn ready(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.ready)
    }

    /// Moves each time an app's install is worked out (a game just stashed, a new build).
    #[must_use]
    pub fn builds_mark(&self) -> u64 {
        self.changes.load(Ordering::Relaxed)
    }

    /// Raises `ready` (made before, to hand to what waits for it) instead of a flag of its own.
    #[must_use]
    pub fn with_ready(mut self, ready: Arc<AtomicBool>) -> Self {
        self.ready = ready;
        self
    }

    /// With the probe of lancache's old cache.
    #[must_use]
    pub fn with_probe(mut self, probe: Arc<SteamLancache>) -> Self {
        self.probe = Some(probe);
        self
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<u32, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The library's apps whose depots the stash holds, by id.
    fn stashed(&self, lib: &Library) -> Vec<u32> {
        let apps: BTreeSet<u32> = self
            .store
            .usage()
            .keys()
            .filter(|g| g.service == "steam")
            .filter_map(|g| lib.depot_app.get(&g.game.parse().ok()?).copied())
            .filter(|a| lib.app(*a).is_some_and(|a| lib.installable(a, &self.target)))
            .collect();
        apps.into_iter().collect()
    }

    /// Reads the install of each stashed app whose build is new (one connection for all).
    ///
    /// # Errors
    /// No library yet, or the connection could not be opened.
    pub fn refresh(&self) -> Result<(), String> {
        let lib = self.library.get().ok_or("no Steam library yet")?;
        let stale: Vec<&App> = self
            .stashed(&lib)
            .into_iter()
            .filter_map(|id| lib.app(id))
            .filter(|a| self.entries().get(&a.id).is_none_or(|e| e.build != a.build))
            .collect();
        // Those whose chunk lists are all kept first, without asking Steam; then the others,
        // with one list of content servers for all.
        let mut servers: Option<Servers> = None;
        let (kept, rest): (Vec<&App>, Vec<&App>) = stale.into_iter().partition(|a| {
            lib.install(a, &self.target)
                .iter()
                .all(|(_, d)| d.manifest.is_some_and(|g| self.services.lists.kept(d.id, g).is_some()))
        });
        for app in kept {
            let chunks = kept_install(&self.services, (&lib, app), &self.target);
            let entry = Entry { build: app.build, chunks: chunks.map(Arc::new), error: None };
            self.entries().insert(app.id, entry);
            self.changes.fetch_add(1, Ordering::Relaxed);
        }
        for app in rest {
            let read = install(&self.services, &mut servers, (&lib, app), &self.target);
            let entry = match read {
                Ok(chunks) => Entry { build: app.build, chunks: Some(Arc::new(chunks)), error: None },
                Err(e) => Entry { build: app.build, chunks: None, error: Some(e) },
            };
            self.entries().insert(app.id, entry);
            self.changes.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
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

    /// The view (`GET /api/v1/builds/steam`).
    #[must_use]
    pub fn view(&self) -> String {
        render(&self.target.os, self.lancache, &self.apps())
    }

    /// Where each stashed app's current build stands (for automatic updates).
    #[must_use]
    pub fn status(&self) -> Vec<lanstash_ports::platform::BuildStatus> {
        self.apps()
            .iter()
            .filter_map(|b| {
                Some(lanstash_ports::platform::BuildStatus {
                    game: b.app.to_string(),
                    version: b.build?.to_string(),
                    known: b.have.is_some() && b.want.is_some(),
                    current: b.current(),
                })
            })
            .collect()
    }

    /// The stashed apps whose current build the stash holds whole.
    #[must_use]
    pub fn current(&self) -> Vec<u32> {
        self.apps().iter().filter(|b| b.current()).map(|b| b.app).collect()
    }

    /// Each stashed app's build and how much of it is held: worked out again only when Steam's part
    /// of the stash, the apps' installs or the library moved (the view and the status, asked one
    /// after the other, share one walk), and always while lancache's old cache is probed (what it
    /// holds says nothing when it changes).
    fn apps(&self) -> Arc<Vec<AppBuild>> {
        let Some(lib) = self.library.get() else { return Arc::new(Vec::new()) };
        if self.probe.is_some() {
            return Arc::new(self.work_out(&lib));
        }
        let key = (self.store.changes("steam"), self.changes.load(Ordering::Relaxed), lib.at_ms);
        self.worked.get(key, || self.work_out(&lib))
    }

    fn work_out(&self, lib: &Arc<Library>) -> Vec<AppBuild> {
        let lib = Arc::clone(lib);
        self.stashed(&lib)
            .into_iter()
            .filter_map(|id| lib.app(id))
            .map(|a| {
                let e = self.entries().get(&a.id).filter(|e| e.build == a.build).cloned();
                let chunks = e.as_ref().and_then(|e| e.chunks.clone());
                let mut b = AppBuild {
                    app: a.id,
                    name: a.name.clone(),
                    build: a.build,
                    have: chunks.as_ref().map(|c| self.held(c)),
                    want: chunks.as_ref().map(|c| c.iter().map(|(_, s)| s).sum()),
                    error: e.and_then(|e| e.error),
                    lancache: None,
                };
                if !b.current() {
                    b.lancache = self.probe.as_ref().and_then(|p| p.share(&lib, a));
                }
                b
            })
            .collect()
    }

    /// What a refresh depends on, cheap to look at: how often Steam's part of the stash changed,
    /// and when the library was read.
    fn mark(&self) -> (u64, u64) {
        (self.store.changes("steam"), self.library.get().map_or(0, |l| l.at_ms))
    }

    /// Keeps the builds fresh on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("steam-builds", Arc::clone(&pause), move || {
            let before = me.mark();
            // Errors show in the view; until the library is read, look again soon.
            match me.refresh() {
                Err(_) if me.library.get().is_none() => pause.pause_ms(SOON_MS),
                r => {
                    if r.is_ok() {
                        me.ready.store(true, Ordering::Release);
                    }
                    // A game just stashed (a prefill, a download) is read at once, not at the
                    // next round: what Steam's part of the stash holds moved.
                    until_changed(pause.as_ref(), CHANGE_LOOK_MS, LOOK_MS, &before, &|| me.mark());
                }
            }
            lanstash_app::rounds::Next::Again
        })
    }
}
