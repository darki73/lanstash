//! The accounts' library: every app their licenses grant, with its depots and
//! current manifests ([`read`]: licenses, then package info, then app access
//! tokens and app info, in batches), per account signed in and merged, with who
//! owns each app and depot ([`from_raws`]; [`Library::account_for`] picks the
//! account to fetch a depot as).
//! [`SteamLibrary`] keeps the latest one,
//! refreshed on a thread of its own, and names Steam's store entries from
//! it: a depot is its app's (exactly, not by the id heuristic), and a DLC's
//! own depot its game's: a DLC is part of its game, for installs and names.
//! Steam's answers can be kept between runs ([`SteamLibrary::with_keep`]), so
//! the library is there at once after a start.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::keep::Keep;
use lanstash_ports::names::GameNames;
use lanstash_steam_domain::apps::{App, Depot, Target, app};
use lanstash_steam_domain::kv::parse_text;

use crate::connection::Connection;
use crate::raw::{Raw, decode_raws, encode_raws};
use crate::session::Services;

/// Every app of the accounts signed in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    pub apps: Vec<App>,
    /// Depot to the app that has it (its own app; a shared depot's provider too).
    pub depot_app: BTreeMap<u32, u32>,
    /// The depots the accounts' licenses grant (whose manifests Steam gives them).
    pub owned_depots: BTreeSet<u32>,
    /// Each app's owners: the accounts whose licenses grant it, in the order they signed in.
    pub app_owners: BTreeMap<u32, Vec<String>>,
    /// Each depot's owners, likewise.
    pub depot_owners: BTreeMap<u32, Vec<String>>,
    /// When it was read (ms since the epoch).
    pub at_ms: u64,
}

/// The library of Steam's answers (an app whose info does not parse is left out).
#[must_use]
pub fn from_raw(raw: &Raw) -> Library {
    let apps = raw.apps.iter().filter_map(|(id, text)| parse_text(text).ok().map(|kv| app(*id, &kv))).collect();
    Library::new(apps, raw.owned.iter().copied().collect())
}

/// The library of several accounts' answers, in the order they signed in: each app once
/// (as the first account that has it sees it), and who owns each app and depot.
#[must_use]
pub fn from_raws(raws: &[(String, Raw)]) -> Library {
    let mut seen = BTreeSet::new();
    let mut apps = Vec::new();
    let (mut app_owners, mut depot_owners) = (BTreeMap::<u32, Vec<String>>::new(), BTreeMap::<u32, Vec<String>>::new());
    for (account, raw) in raws {
        for (id, text) in &raw.apps {
            app_owners.entry(*id).or_default().push(account.clone());
            if seen.insert(*id)
                && let Ok(kv) = parse_text(text)
            {
                apps.push(app(*id, &kv));
            }
        }
        for d in &raw.owned {
            depot_owners.entry(*d).or_default().push(account.clone());
        }
    }
    let owned = depot_owners.keys().copied().collect();
    Library { app_owners, depot_owners, ..Library::new(apps, owned) }
}

/// Reads the library through a logged-on connection ([`crate::read::read_raw`]).
///
/// # Errors
/// The connection, or an answer that does not parse.
pub fn read(conn: &mut Connection) -> Result<Library, String> {
    crate::read::read_raw(conn).map(|r| from_raw(&r))
}

/// A depot an install downloads: the app whose manifest it is (its provider, for a
/// shared one) and the depot.
pub type InstallDepot<'a> = (u32, &'a Depot);

impl Library {
    /// The library of `apps`: sorted by id, each depot filed under the app it
    /// belongs to: its provider for a shared one, the game for a DLC's own
    /// depot (so a DLC is part of its game), else the app that lists it.
    #[must_use]
    pub fn new(mut apps: Vec<App>, owned_depots: BTreeSet<u32>) -> Self {
        apps.sort_by_key(|a| a.id);
        let mut lib = Self { apps, owned_depots, ..Self::default() };
        let mut depot_app = BTreeMap::new();
        for a in &lib.apps {
            let home = lib.game_of(a).map_or(a.id, |g| g.id);
            for d in &a.depots {
                depot_app.entry(d.id).or_insert(d.from_app.unwrap_or(home));
            }
        }
        lib.depot_app = depot_app;
        lib
    }

    /// The game a DLC of the library belongs to, when the library has it.
    #[must_use]
    pub fn game_of(&self, dlc: &App) -> Option<&App> {
        let by_parent = dlc.parent.and_then(|p| self.app(p)).filter(|g| dlc.is_dlc_of(g));
        by_parent.or_else(|| self.apps.iter().find(|g| g.dlcs.contains(&dlc.id) && dlc.is_dlc_of(g)))
    }

    /// What an install of `app` for `t` downloads: its own depots, those of its
    /// DLCs the account owns (in the library), listed with the game or with the
    /// DLC itself (fetched as the DLC's), and the shared ones (fetched as their
    /// provider: its entry when the library has it, else the one `app` lists).
    /// Only depots the accounts' licenses grant ([`Self::grants`]): Steam refuses
    /// the rest ("access denied"). Each depot once.
    #[must_use]
    pub fn install<'a>(&'a self, app: &'a App, t: &Target) -> Vec<InstallDepot<'a>> {
        let own = app.install(t).into_iter().map(|d| (app.id, d));
        let shared = app.shared(t).into_iter().filter_map(|(provider, depot)| {
            let theirs = self.app(provider).and_then(|p| p.depots.iter().find(|d| d.id == depot && d.has_content()));
            let listed = || app.depots.iter().find(|d| d.id == depot && d.has_content());
            Some((provider, theirs.or_else(listed)?))
        });
        let dlc = app.dlc_depots(t).into_iter().filter(|(x, _)| self.app(*x).is_some()).map(|(_, d)| (app.id, d));
        let dlc_own = self
            .apps
            .iter()
            .filter(|x| x.is_dlc_of(app) && self.game_of(x).is_some_and(|g| g.id == app.id))
            .flat_map(|x| x.dlc_content(t).into_iter().map(move |d| (x.id, d)));
        let mut seen = BTreeSet::new();
        own.chain(dlc).chain(dlc_own).chain(shared).filter(|(_, d)| self.grants(d.id) && seen.insert(d.id)).collect()
    }

    /// Whether the accounts' licenses grant `depot`: a package lists it among its
    /// depots, or among its apps (a depot whose id is a granted app's, as many of
    /// Valve's older ones are). Apps often list depots no license grants either way.
    #[must_use]
    pub fn grants(&self, depot: u32) -> bool {
        self.owned_depots.contains(&depot) || self.app(depot).is_some()
    }

    /// Whether an install of `app` for `t` downloads anything.
    #[must_use]
    pub fn installable(&self, app: &App, t: &Target) -> bool {
        !self.install(app, t).is_empty()
    }

    #[must_use]
    pub fn app(&self, id: u32) -> Option<&App> {
        self.apps.binary_search_by_key(&id, |a| a.id).ok().map(|i| &self.apps[i])
    }

    /// The account to fetch `depot` of `app` as: the first of `accounts` (those signed in, in
    /// order) whose licenses grant the depot, else one that owns the app, else the first one.
    #[must_use]
    pub fn account_for(&self, app: u32, depot: u32, accounts: &[String]) -> Option<String> {
        let owns = |owners: Option<&Vec<String>>| {
            accounts.iter().find(|a| owners.is_some_and(|o| o.iter().any(|x| x.eq_ignore_ascii_case(a)))).cloned()
        };
        owns(self.depot_owners.get(&depot))
            .or_else(|| owns(self.app_owners.get(&app)))
            .or_else(|| accounts.first().cloned())
    }

    /// `install`'s depots by the account to fetch each as ([`Self::account_for`]), in the
    /// order the accounts signed in.
    #[must_use]
    pub fn by_account<'a>(
        &self,
        install: &[InstallDepot<'a>],
        accounts: &[String],
    ) -> Vec<(String, Vec<InstallDepot<'a>>)> {
        let mut groups: Vec<(String, Vec<InstallDepot<'a>>)> = Vec::new();
        for (owner, d) in install {
            let Some(account) = self.account_for(*owner, d.id, accounts) else { continue };
            match groups.iter_mut().find(|(a, _)| *a == account) {
                Some((_, ds)) => ds.push((*owner, *d)),
                None => groups.push((account, vec![(*owner, *d)])),
            }
        }
        groups.sort_by_key(|(a, _)| accounts.iter().position(|x| x == a));
        groups
    }
}

/// How often the library is read again.
pub const REFRESH_MS: u64 = 6 * 3600 * 1000;
/// How often the refresh thread looks whether it is time (and whether a sign-in appeared).
const LOOK_MS: u64 = 60 * 1000;

/// The latest library of every account signed in, refreshed in the background.
pub struct SteamLibrary {
    services: Services,
    clock: Arc<dyn Clock>,
    latest: Mutex<Option<Arc<Library>>>,
    /// Each account's last answers (one whose read fails keeps its last).
    raws: Mutex<Vec<(String, Raw)>>,
    /// Why the last read failed, if it did (for some accounts or all).
    error: Mutex<Option<String>>,
    /// Where the last read is kept between runs.
    keep: Option<Arc<dyn Keep>>,
}

/// The name the last read is kept under.
pub const KEPT: &str = "steam-library";

impl std::fmt::Debug for SteamLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamLibrary").finish_non_exhaustive()
    }
}

impl SteamLibrary {
    #[must_use]
    pub fn new(services: Services, clock: Arc<dyn Clock>) -> Self {
        Self {
            services,
            clock,
            latest: Mutex::new(None),
            raws: Mutex::new(Vec::new()),
            error: Mutex::new(None),
            keep: None,
        }
    }

    /// Keeps each read in `keep`, and starts from the one kept there: the library is there at once
    /// after a start, read again when it is as old as [`REFRESH_MS`] (counted from when it was read)
    /// or the accounts signed in changed. A single-account one (`LSL1`) is the first account's.
    #[must_use]
    pub fn with_keep(mut self, keep: Arc<dyn Keep>) -> Self {
        let first = self.services.accounts().into_iter().next();
        if let Some((raws, at_ms)) = keep.load(KEPT).ok().flatten().and_then(|b| decode_raws(&b, first.as_deref())) {
            let mut lib = from_raws(&raws);
            lib.at_ms = at_ms;
            *self.latest.get_mut().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(lib));
            *self.raws.get_mut().unwrap_or_else(PoisonError::into_inner) = raws;
        }
        self.keep = Some(keep);
        self
    }

    /// The latest library, if one was read.
    #[must_use]
    pub fn get(&self) -> Option<Arc<Library>> {
        self.latest.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Why the last read failed.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Reads every signed-in account's library now and merges them. An account whose read
    /// fails keeps its last answers (and the failure is kept for the API to show); one no
    /// longer signed in is dropped.
    ///
    /// # Errors
    /// No sign-in, or no account's library read nor kept.
    pub fn refresh(&self) -> Result<Arc<Library>, String> {
        let accounts = self.services.accounts();
        if accounts.is_empty() {
            let e = String::from("not signed in to Steam: sign in from the web UI");
            *self.error.lock().unwrap_or_else(PoisonError::into_inner) = Some(e.clone());
            return Err(e);
        }
        let before = self.raws.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let mut raws = Vec::new();
        let mut failed = Vec::new();
        for account in &accounts {
            match self.services.with_session_as(account, |c, _| crate::read::read_raw(c)) {
                Ok(raw) => raws.push((account.clone(), raw)),
                Err(e) => {
                    failed.push(format!("{account}: {e}"));
                    if let Some(old) = before.iter().find(|(a, _)| a.eq_ignore_ascii_case(account)) {
                        raws.push(old.clone());
                    }
                }
            }
        }
        let error = (!failed.is_empty()).then(|| failed.join("; "));
        *self.error.lock().unwrap_or_else(PoisonError::into_inner) = error.clone();
        if raws.is_empty() {
            return Err(error.unwrap_or_default());
        }
        let mut lib = from_raws(&raws);
        lib.at_ms = self.clock.now_ms();
        if let Some(k) = &self.keep {
            // Losing it costs a read of the library at the next start, never correctness.
            let _ = k.save(KEPT, &encode_raws(&raws, lib.at_ms));
        }
        *self.raws.lock().unwrap_or_else(PoisonError::into_inner) = raws;
        let lib = Arc::new(lib);
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&lib));
        Ok(lib)
    }

    /// Whether it is time to read again: signed in, and none yet, an old one, or one of
    /// other accounts than those signed in now.
    fn due(&self) -> bool {
        let accounts = self.services.accounts();
        if accounts.is_empty() {
            return false;
        }
        let read: Vec<String> = self
            .raws
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(a, _)| a.to_ascii_lowercase())
            .collect();
        let now: Vec<String> = accounts.iter().map(|a| a.to_ascii_lowercase()).collect();
        read != now || self.get().is_none_or(|l| self.clock.now_ms().saturating_sub(l.at_ms) >= REFRESH_MS)
    }

    /// Keeps the library fresh on a thread of its own (for as long as the process runs).
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("steam-library", Arc::clone(&pause), move || {
            if me.due() {
                // A failure is kept for the API to show; the next look tries again.
                let _ = me.refresh();
            }
            pause.pause_ms(LOOK_MS);
            lanstash_app::rounds::Next::Again
        })
    }
}

impl GameNames for SteamLibrary {
    /// A depot (or app) of the library: its app's name.
    fn name(&self, service: &str, game: &str) -> Option<String> {
        if service != "steam" {
            return None;
        }
        let lib = self.get()?;
        let id: u32 = game.parse().ok()?;
        let app = lib.depot_app.get(&id).copied().unwrap_or(id);
        lib.app(app)?.name.clone()
    }

    /// A depot's app.
    fn group(&self, service: &str, game: &str) -> Option<String> {
        if service != "steam" {
            return None;
        }
        let lib = self.get()?;
        lib.depot_app.get(&game.parse().ok()?).map(ToString::to_string)
    }
}
