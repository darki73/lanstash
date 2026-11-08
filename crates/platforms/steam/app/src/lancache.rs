//! How much of each library game lancache's old cache holds: per app and
//! build, the current manifests are read and a sample of the install's
//! chunks looked up in the cache directory ([`lanstash_steam_domain::share`]),
//! on a thread of its own, one app at a time on the shared session, once per build.
//! The results are kept ([`Keep`], one line per app) so a restart does not
//! probe the whole library again. A manifest's kept chunk list serves when
//! there is one. After a start it can wait until the stashed apps' builds
//! were worked out ([`SteamLancache::after`]), so those go first.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Pause;
use lanstash_ports::keep::Keep;
use lanstash_ports::nginx::NginxCacheSource;
use lanstash_steam_domain::apps::{App, Target};
use lanstash_steam_domain::catalog::offered;
use lanstash_steam_domain::share::{estimate, first_slice, sample};

use crate::library::{Library, SteamLibrary};
use crate::manifests::{self, Servers};
use crate::session::Services;

/// Why a pass did not probe: what goes first is not done yet.
const WAITING: &str = "waiting for the stashed apps' builds";

/// Chunks looked up per app.
const SAMPLE: usize = 256;
/// How long between two passes over the library, and while there is no library yet.
const PASS_MS: u64 = 10 * 60 * 1000;
const SOON_MS: u64 = 30 * 1000;
/// A break between two apps: the session's lock is not fair, and without it the
/// probe would take the session straight back from the builds view or a prefill.
const BETWEEN_MS: u64 = 500;

/// What lancache holds of an app's build (an estimate), in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Share {
    pub build: Option<u64>,
    pub have: u64,
    pub want: u64,
}

/// The probes' results.
pub struct SteamLancache {
    services: Services,
    library: Arc<SteamLibrary>,
    source: Arc<dyn NginxCacheSource>,
    target: Target,
    results: Mutex<BTreeMap<u32, Share>>,
    keep: Option<Arc<dyn Keep>>,
    /// Raised when what goes first is done (the stashed apps' builds after a start); none: no waiting.
    after: Option<Arc<AtomicBool>>,
}

/// The name the results are kept under.
pub const KEPT: &str = "steam-lancache";

/// The results as lines: `app build have want` (`-` for no build).
#[must_use]
pub fn encode(results: &BTreeMap<u32, Share>) -> String {
    results
        .iter()
        .map(|(app, s)| {
            let build = s.build.map_or_else(|| "-".to_string(), |b| b.to_string());
            format!(
                "{app} {build} {} {}
",
                s.have, s.want
            )
        })
        .collect()
}

/// Reads [`encode`]'s lines back (a line that does not read is skipped).
#[must_use]
pub fn decode(text: &str) -> BTreeMap<u32, Share> {
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let app = w.next()?.parse().ok()?;
            let build = match w.next()? {
                "-" => None,
                b => Some(b.parse().ok()?),
            };
            Some((app, Share { build, have: w.next()?.parse().ok()?, want: w.next()?.parse().ok()? }))
        })
        .collect()
}

impl std::fmt::Debug for SteamLancache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamLancache").finish_non_exhaustive()
    }
}

impl SteamLancache {
    #[must_use]
    pub fn new(
        services: Services,
        library: Arc<SteamLibrary>,
        source: Arc<dyn NginxCacheSource>,
        target: Target,
    ) -> Self {
        Self { services, library, source, target, results: Mutex::new(BTreeMap::new()), keep: None, after: None }
    }

    /// Probes only once `after` is raised (so the stashed apps' builds go first on the shared session).
    #[must_use]
    pub fn after(mut self, after: Arc<AtomicBool>) -> Self {
        self.after = Some(after);
        self
    }

    /// Keeps the results in `keep` (and starts from those kept there).
    #[must_use]
    pub fn with_keep(mut self, keep: Arc<dyn Keep>) -> Self {
        if let Ok(Some(b)) = keep.load(KEPT) {
            *self.results.get_mut().unwrap_or_else(PoisonError::into_inner) = decode(&String::from_utf8_lossy(&b));
        }
        self.keep = Some(keep);
        self
    }

    fn save(&self) {
        if let Some(k) = &self.keep {
            // Losing the results costs a new probe, never correctness.
            let _ = k.save(KEPT, encode(&self.results()).as_bytes());
        }
    }

    fn results(&self) -> std::sync::MutexGuard<'_, BTreeMap<u32, Share>> {
        self.results.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What lancache holds of `app`'s current install (have, want), once probed:
    /// an estimate of another build, or of an install of another size (owned DLCs
    /// changed, say), does not count.
    #[must_use]
    pub fn share(&self, lib: &Library, app: &App) -> Option<(u64, u64)> {
        let size = Self::size(lib, app, &self.target);
        self.results()
            .get(&app.id)
            .filter(|s| s.build == app.build && s.want > 0 && s.want == size)
            .map(|s| (s.have, s.want))
    }

    /// Bytes an install of `app` downloads, as the app info gives them.
    fn size(lib: &Library, app: &App, t: &Target) -> u64 {
        lib.install(app, t).iter().map(|(_, d)| d.download).sum()
    }

    /// Probes one app: its install's chunks, a sample of them looked up.
    fn probe(&self, servers: &mut Option<Servers>, (lib, app): (&Library, &App)) -> Result<Share, String> {
        // A kept list serves; the probe keeps none of its own (its results are kept instead).
        let all = manifests::install_chunks(&self.services, servers, lib, &lib.install(app, &self.target), false)?;
        let want: u64 = all.iter().map(|(_, c)| u64::from(c.compressed)).sum();
        let (mut sampled, mut found) = (0u64, 0u64);
        for (depot, c) in sample(&all, SAMPLE) {
            sampled += u64::from(c.compressed);
            if self.source.exists(&first_slice(*depot, c)).map_err(|e| e.to_string())? {
                found += u64::from(c.compressed);
            }
        }
        Ok(Share { build: app.build, have: estimate(want, sampled, found), want })
    }

    /// One pass: probes every offered app whose current build is not probed yet,
    /// pausing between two; returns how many.
    ///
    /// # Errors
    /// No library yet, or a connection could not be opened.
    pub fn pass(&self, pause: &dyn Pause) -> Result<usize, String> {
        if self.after.as_ref().is_some_and(|a| !a.load(Ordering::Acquire)) {
            return Err(WAITING.into());
        }
        let lib = self.library.get().ok_or("no Steam library yet")?;
        let todo: Vec<&App> = lib
            .apps
            .iter()
            .filter(|a| offered(a, &self.target))
            .filter(|a| {
                let size = Self::size(&lib, a, &self.target);
                self.results().get(&a.id).is_none_or(|s| s.build != a.build || (s.want > 0 && s.want != size))
            })
            .collect();
        let mut servers = None;
        for app in &todo {
            // A failed probe counts as nothing held, so the pass does not retry it until the next build.
            let probed = self.probe(&mut servers, (&lib, app));
            self.results().insert(app.id, probed.unwrap_or(Share { build: app.build, have: 0, want: 0 }));
            self.save();
            pause.pause_ms(BETWEEN_MS);
        }
        Ok(todo.len())
    }

    /// Keeps probing on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("steam-lancache", Arc::clone(&pause), move || {
            let wait = match me.pass(pause.as_ref()) {
                Err(e) if me.library.get().is_none() || e == WAITING => SOON_MS,
                _ => PASS_MS,
            };
            pause.pause_ms(wait);
            lanstash_app::rounds::Next::Again
        })
    }
}
