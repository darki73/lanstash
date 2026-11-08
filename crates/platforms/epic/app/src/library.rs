//! The Epic accounts' library ([`EpicLibrary`]): each signed-in account's
//! assets, read from Epic's launcher service as that account, and the catalog
//! items they are (title, game or not, images), each item asked of Epic's
//! catalog once and kept. It is read again every few hours, and soon after the
//! accounts signed in change; what was read is kept between runs, so names and
//! art are there at once after a start. It names the games of the stash
//! ([`GameNames`]: an app's title), finds their images ([`ArtSource`]), and
//! renders the catalog view: every game an account owns, with its owners.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::saved::{Saved, decode, encode};
use lanstash_epic::library::{
    ASSETS_PATH, Asset, CATALOG_HOST, Game, Item, LAUNCHER_HOST, LIBRARY_HOST, image, image_source, items_path,
    manifest_path, parse_assets, parse_items, parse_manifest_dirs, parse_records, records_path, render_catalog,
    render_none,
};
use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::clock::{Clock, Pause};
use lanstash_ports::keep::Keep;
use lanstash_ports::names::GameNames;
use lanstash_ports::upstream::Source;
use lanstash_ports::web::{Tls, Web, WebRequest};

use crate::accounts::EpicAccounts;

/// How often the library is read again.
pub const REFRESH_MS: u64 = 6 * 3600 * 1000;
/// How often the refresh thread looks whether it is time.
const LOOK_MS: u64 = 60 * 1000;
/// The name the library is kept under (`-2`: with the library service's apps; the first kind,
/// without them, is dropped so it is read again).
pub const KEPT: &str = "epic-library-2";
const KEPT_BEFORE: &str = "epic-library";
/// Pages of an account's library read at most.
const MAX_PAGES: usize = 50;

#[derive(Debug, Default)]
struct State {
    /// Each account's assets (by account id), in the order they signed in.
    assets: Vec<(String, Vec<Asset>)>,
    /// Catalog items by id.
    items: BTreeMap<String, Item>,
    /// The directory an app's downloads are filed under, to the app (they differ: a hash, a name).
    dirs: BTreeMap<String, String>,
    /// Apps whose directories were asked for this run (found or not), not asked again.
    tried: std::collections::BTreeSet<String>,
    at_ms: u64,
    error: Option<String>,
}

/// The Epic accounts' library.
pub struct EpicLibrary {
    accounts: Arc<EpicAccounts>,
    web: Arc<dyn Web>,
    clock: Arc<dyn Clock>,
    keep: Option<Arc<dyn Keep>>,
    state: Mutex<State>,
}

impl std::fmt::Debug for EpicLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpicLibrary").finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl EpicLibrary {
    #[must_use]
    pub fn new(accounts: Arc<EpicAccounts>, web: Arc<dyn Web>, clock: Arc<dyn Clock>) -> Self {
        Self { accounts, web, clock, keep: None, state: Mutex::default() }
    }

    /// Keeps each read in `keep`, and starts from the one kept there.
    #[must_use]
    pub fn with_keep(mut self, keep: Arc<dyn Keep>) -> Self {
        // Losing it costs nothing: it was read again anyway.
        let _ = keep.remove(KEPT_BEFORE);
        if let Some(s) = keep.load(KEPT).ok().flatten().and_then(|b| decode(&String::from_utf8_lossy(&b))) {
            let state = State { assets: s.assets, items: s.items, dirs: s.dirs, at_ms: s.at_ms, ..State::default() };
            *self.state.get_mut().unwrap_or_else(PoisonError::into_inner) = state;
        }
        self.keep = Some(keep);
        self
    }

    /// A GET of Epic's `host` and `target` as `account`: the body.
    fn get(&self, account: &str, host: &str, target: &str) -> Result<String, String> {
        let token = self.accounts.access_token(account)?;
        let auth = format!("bearer {token}");
        let headers = [("Authorization", auth.as_str()), ("Accept", "application/json")];
        let req = WebRequest {
            method: "GET",
            host,
            port: 443,
            target,
            headers: &headers,
            body: &[],
            tls: Some(Tls { pin: None, unchecked: false }),
        };
        let answer = self.web.send(&req).map_err(|e| format!("Epic: {e}"))?;
        let body = String::from_utf8_lossy(&answer.body).into_owned();
        if answer.status == 200 { Ok(body) } else { Err(format!("Epic answered {} for {target}", answer.status)) }
    }

    /// Every app of `account`'s library, page by page (at most [`MAX_PAGES`]).
    fn records(&self, account: &str) -> Result<Vec<Asset>, String> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let (apps, next) = parse_records(&self.get(account, LIBRARY_HOST, &records_path(cursor.as_deref()))?)?;
            all.extend(apps);
            match next {
                Some(c) if Some(&c) != cursor.as_ref() => cursor = Some(c),
                _ => break,
            }
        }
        Ok(all)
    }

    /// Reads every signed-in account's assets and the catalog items not known yet. An account
    /// whose read fails keeps its last assets (and the failure is kept for the view); one no
    /// longer signed in is dropped.
    ///
    /// # Errors
    /// No account signed in.
    pub fn refresh(&self) -> Result<(), String> {
        let accounts = self.accounts.accounts();
        if accounts.is_empty() {
            return Err("no Epic account signed in".into());
        }
        let before = lock(&self.state).assets.clone();
        let mut assets = Vec::new();
        let mut failed = Vec::new();
        for k in &accounts {
            match self.get(&k.id, LAUNCHER_HOST, ASSETS_PATH).and_then(|b| parse_assets(&b)) {
                Ok(mut list) => {
                    // The library service lists what the launcher's assets leave out (its builds unknown).
                    match self.records(&k.id) {
                        Ok(more) => {
                            for a in more {
                                if !list.iter().any(|x| x.app_name.eq_ignore_ascii_case(&a.app_name)) {
                                    list.push(a);
                                }
                            }
                        }
                        Err(e) => failed.push(format!("{}: {e}", k.name)),
                    }
                    assets.push((k.id.clone(), list));
                }
                Err(e) => {
                    failed.push(format!("{}: {e}", k.name));
                    if let Some(old) = before.iter().find(|(a, _)| *a == k.id) {
                        assets.push(old.clone());
                    }
                }
            }
        }
        // Items not known yet, asked once each (as the first account that has them).
        let known: Vec<String> = lock(&self.state).items.keys().cloned().collect();
        let mut items = BTreeMap::new();
        for (account, list) in &assets {
            for a in list {
                if known.contains(&a.catalog_item_id) || items.contains_key(&a.catalog_item_id) {
                    continue;
                }
                let Some(path) = items_path(&a.namespace, &a.catalog_item_id) else { continue };
                match self.get(account, CATALOG_HOST, &path).and_then(|b| parse_items(&b)) {
                    Ok(found) => items.extend(found),
                    Err(e) => failed.push(format!("{}: {e}", a.app_name)),
                }
            }
        }
        let mut s = lock(&self.state);
        s.assets = assets;
        s.items.extend(items);
        s.at_ms = self.clock.now_ms();
        s.error = (!failed.is_empty()).then(|| failed.join("; "));
        self.save(&s);
        drop(s);
        self.find_dirs();
        Ok(())
    }

    fn save(&self, s: &State) {
        if let Some(k) = &self.keep {
            // Losing it costs a read of the library at the next start, never correctness.
            let saved =
                Saved { assets: s.assets.clone(), items: s.items.clone(), dirs: s.dirs.clone(), at_ms: s.at_ms };
            let _ = k.save(KEPT, encode(&saved).as_bytes());
        }
    }

    /// The apps (each with an account that owns it) whose download directories are not known
    /// and not asked for yet this run: those with a build (one to download).
    fn missing_dirs(&self) -> Vec<(String, Asset)> {
        let s = lock(&self.state);
        let mut out: Vec<(String, Asset)> = Vec::new();
        for (account, list) in &s.assets {
            for a in list {
                let known = s.dirs.values().any(|x| x == &a.app_name) || s.tried.contains(&a.app_name);
                if !a.build_version.is_empty() && !known && !out.iter().any(|(_, x)| x.app_name == a.app_name) {
                    out.push((account.clone(), a.clone()));
                }
            }
        }
        out
    }

    /// Asks Epic where each app's downloads are filed (its manifests' directory), for those not known.
    pub fn find_dirs(&self) {
        let missing = self.missing_dirs();
        if missing.is_empty() {
            return;
        }
        let mut found = Vec::new();
        for (account, a) in &missing {
            let Some(path) = manifest_path(a) else { continue };
            if let Ok(body) = self.get(account, LAUNCHER_HOST, &path) {
                found.extend(parse_manifest_dirs(&body).into_iter().map(|d| (d, a.app_name.clone())));
            }
        }
        let mut s = lock(&self.state);
        s.tried.extend(missing.into_iter().map(|(_, a)| a.app_name));
        s.dirs.extend(found);
        self.save(&s);
    }

    /// Whether it is time to read again: signed in, and never read, an old read, or a read of
    /// other accounts than those signed in now.
    fn due(&self) -> bool {
        let now: Vec<String> = self.accounts.accounts().into_iter().map(|k| k.id).collect();
        if now.is_empty() {
            return false;
        }
        let s = lock(&self.state);
        let read: Vec<&String> = s.assets.iter().map(|(a, _)| a).collect();
        read != now.iter().collect::<Vec<_>>() || self.clock.now_ms().saturating_sub(s.at_ms) >= REFRESH_MS
    }

    /// Keeps the library fresh on a thread of its own.
    ///
    /// # Errors
    /// The thread could not be spawned.
    pub fn keep_fresh(self: &Arc<Self>, pause: Arc<dyn Pause>) -> Result<(), String> {
        let me = Arc::clone(self);
        lanstash_app::rounds::forever("epic-library", Arc::clone(&pause), move || {
            if me.due() {
                // A failure is kept for the view; the next look tries again.
                if let Err(e) = me.refresh() {
                    lock(&me.state).error = Some(e);
                }
            } else {
                // A kept library from before the directories were asked for.
                me.find_dirs();
            }
            pause.pause_ms(LOOK_MS);
            lanstash_app::rounds::Next::Again
        })
    }

    /// The app `game` is (its name, or its download directory) with an account that owns it.
    #[must_use]
    pub fn app(&self, game: &str) -> Option<(String, Asset)> {
        let s = lock(&self.state);
        let app = s.dirs.iter().find(|(d, _)| d.eq_ignore_ascii_case(game)).map_or(game, |(_, a)| a.as_str());
        s.assets.iter().find_map(|(account, list)| {
            list.iter().find(|a| a.app_name.eq_ignore_ascii_case(app)).map(|a| (account.clone(), a.clone()))
        })
    }

    /// Epic's answer about `asset`'s current build (its manifests), asked as `account`.
    ///
    /// # Errors
    /// Not an app Epic can be asked about, or Epic's answer.
    pub fn build(&self, account: &str, asset: &Asset) -> Result<String, String> {
        let path = manifest_path(asset).ok_or("not an app Epic can be asked about")?;
        self.get(account, LAUNCHER_HOST, &path)
    }

    /// The item an app is, where its app is in the library.
    fn item_of(&self, app: &str) -> Option<Item> {
        let s = lock(&self.state);
        // A download directory names its app.
        let app = s.dirs.iter().find(|(d, _)| d.eq_ignore_ascii_case(app)).map_or(app, |(_, a)| a.as_str());
        let asset = s.assets.iter().flat_map(|(_, l)| l).find(|a| a.app_name.eq_ignore_ascii_case(app))?;
        s.items.get(&asset.catalog_item_id).cloned()
    }

    /// The catalog view: every game an account owns, each once, with its owners (by name).
    #[must_use]
    pub fn view(&self, size: &dyn Fn(&Asset) -> Option<u64>) -> String {
        let s = lock(&self.state);
        if s.assets.is_empty() {
            return render_none(s.error.as_deref().unwrap_or("reading the Epic library"));
        }
        let names: BTreeMap<String, String> = self.accounts.accounts().into_iter().map(|k| (k.id, k.name)).collect();
        let mut games: Vec<Game<'_>> = Vec::new();
        for (account, list) in &s.assets {
            let owner = names.get(account).cloned().unwrap_or_else(|| account.clone());
            for a in list {
                let Some(item) = s.items.get(&a.catalog_item_id).filter(|i| i.game) else { continue };
                match games.iter_mut().find(|g| g.asset.app_name == a.app_name) {
                    Some(g) => g.owners.push(owner.clone()),
                    None => games.push(Game {
                        asset: a,
                        title: &item.title,
                        owners: vec![owner.clone()],
                        download: size(a),
                    }),
                }
            }
        }
        render_catalog(&games)
    }

    /// Every game of the library, each app once (as the first account that has it lists it).
    #[must_use]
    pub fn games(&self) -> Vec<Asset> {
        let s = lock(&self.state);
        let mut out: Vec<Asset> = Vec::new();
        for a in s.assets.iter().flat_map(|(_, list)| list) {
            let game = s.items.get(&a.catalog_item_id).is_some_and(|i| i.game);
            if game && !out.iter().any(|x| x.app_name == a.app_name) {
                out.push(a.clone());
            }
        }
        out
    }
}

impl GameNames for EpicLibrary {
    /// An app of the library: its title.
    fn name(&self, service: &str, game: &str) -> Option<String> {
        if service != "epicgames" {
            return None;
        }
        self.item_of(game).map(|i| i.title).filter(|t| !t.is_empty())
    }

    /// A download directory's app (they differ: Epic files an app's downloads under another id).
    fn group(&self, service: &str, game: &str) -> Option<String> {
        if service != "epicgames" {
            return None;
        }
        self.app(game).map(|(_, a)| a.app_name)
    }
}

impl ArtSource for EpicLibrary {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        let item = self.item_of(id).ok_or(ArtError::Unknown)?;
        image(&item, kind).and_then(image_source).ok_or(ArtError::Missing)
    }

    /// Epic's image CDN answers only over HTTPS (403 over HTTP): fetched here.
    fn fetch(&self, id: &str, kind: ArtKind) -> Option<Result<Vec<u8>, ArtError>> {
        let src = match self.locate(id, kind) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        let req = WebRequest {
            method: "GET",
            host: &src.host,
            port: 443,
            target: &src.target,
            headers: &[],
            body: &[],
            tls: Some(Tls { pin: None, unchecked: false }),
        };
        Some(match self.web.send(&req) {
            Ok(r) if r.status == 200 => Ok(r.body),
            Ok(r) if r.status == 404 => Err(ArtError::Missing),
            Ok(r) => Err(ArtError::Upstream(format!("{}{}: status {}", src.host, src.target, r.status))),
            Err(e) => Err(ArtError::Upstream(e.to_string())),
        })
    }
}
