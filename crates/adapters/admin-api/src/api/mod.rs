//! The admin port (`admin_listen`): the HTTP API and the built-in status
//! page. Platform paths are answered by the registered [`Platform`] whose id
//! the path names; the platform renders its own documents.
//!
//! API, version 1 (JSON; errors are `{"error": "...", "kind": "..."}`):
//! - `GET /api/v1/version`: `{"version": "..."}`, and `"latest"` when a newer release is known ([`version`]),
//! - `GET /api/v1/status`: one status snapshot,
//! - `GET /api/v1/events`: server-sent events, one snapshot per interval,
//! - `GET /api/v1/art/<platform>/<id>[/icon|/background|/logo]`: a game's cover image, icon, background or
//!   logo, fetched once from the platform's CDN and kept in memory,
//! - `GET /api/v1/builds/<platform>`: each game's current build and how much of its install the stash holds,
//! - `GET /api/v1/catalog/<platform>`: every game the platform offers,
//! - `GET /api/v1/games/<platform>/<game>`: one game's builds in detail, `.../<game>/nginx`: what lancache's
//!   nginx cache holds of it,
//! - `POST /api/v1/prefill/<platform>/<game>[?build=pre]`: start prefilling a game's current (or pre-release)
//!   build (202; 409 `busy` while one runs), `POST /api/v1/prefill/stop`: stop every prefill, `GET
//!   /api/v1/prefill`: the latest prefill (also in every status snapshot),
//! - `DELETE /api/v1/games/<platform>/<game>`: start deleting what the stash holds of a game (202; 409 `busy`
//!   while the platform prefills it or another deletion runs; its progress is in every status snapshot),
//! - `/api/v1/account/<platform>[/qr.svg|/sign-in|/sign-out]`: a platform's account and its sign-in ([`account`]),
//! - `GET /api/v1/dns`, `POST /api/v1/dns/apply`: the network's DNS server's rewrites against lanstash's names,
//!   and what went to the internet ([`dns`]),
//! - `GET /api/v1/setup/<platform>/<os>`: the file that sets up a PC running `os` (`windows`, `macos`, `linux`)
//!   for a platform, to save ([`setup`]),
//! - `GET /api/v1/clients`, `PUT|DELETE /api/v1/clients/<address>`: what each client got, and the names given
//!   to clients ([`clients`]),
//! - `GET /api/v1/private`, `PUT|DELETE /api/v1/private/<platform>/<game>`: a player's own games
//!   kept to itself ([`private`]),
//! - `GET /api/v1/hidden`, `PUT|DELETE /api/v1/hidden/<platform>/<game>`: the games kept private ([`hidden`]),
//! - `POST /api/v1/shutdown`: stop cleanly; from this machine only.
//!
//! `/` is the web UI when its build is given ([`web`]), else the built-in page (`status_page.html`, compiled
//! in), drawn from the event stream. `/status.json`, `/events` and `/shutdown` are the paths from before the
//! API, kept so that a newer `lanstash stop` or `doctor` and an older running lanstash still understand each
//! other during an upgrade.

pub mod access;
mod account;
mod answer;
use answer::Answer;
mod clients;
pub mod dns;
pub mod events;
pub mod health;
mod hidden;
use health::HealthOf;
mod history;
pub mod invites;
mod jobs;
mod kept;
pub mod own;
mod private;
pub mod settings;
mod setup;
mod updates;
pub mod users;
pub mod version;
mod web;

use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lanstash_app::art::Art;
use lanstash_app::build_history::BuildHistory;
use lanstash_app::client_names::ClientNames;
use lanstash_app::counters_kept::CountersKept;
use lanstash_app::delete_game::GameDeleter;
use lanstash_app::dns::DnsLink;
use lanstash_app::prefill_runs::PrefillRuns;
use lanstash_app::serve::ConnState;
use lanstash_app::status::{CounterGroup, Status};
use lanstash_ports::art::{ArtError, ArtKind};
use lanstash_ports::platform::Platform;
use lanstash_ports::prefill::{Prefill, PrefillStatus};
use lanstash_ports::settings::Tunables;

use crate::render::{Sample, rates, render_json};
use crate::route::{ART, BUILDS, CATALOG, GAMES, Route, is_api, rest, valid_game};

const PAGE: &str = include_str!("../status_page.html");

/// The version `GET /api/v1/version` reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What `POST /api/v1/shutdown` runs.
pub type StopHook = Arc<dyn Fn() + Send + Sync>;

/// Counters of a part outside the cache (the HTTPS passthrough), by name, read at each snapshot.
pub type Counters = Arc<dyn Fn() -> Vec<(&'static str, u64)> + Send + Sync>;

pub struct AdminApi {
    status: Arc<Status>,
    /// Further counter groups of every snapshot.
    counters: Vec<(&'static str, Counters)>,
    interval: Duration,
    /// For `GET /api/v1/status` callers (each event stream keeps its own).
    last: Mutex<Option<Sample>>,
    /// Runs on a shutdown request from this machine (the composition root stops the process).
    stop: Option<StopHook>,
    /// Cover art; without it the art paths answer 404.
    art: Option<Arc<Art>>,
    /// The platforms the platform paths name.
    platforms: Vec<Arc<dyn Platform>>,
    /// Deletes games; without it the delete path answers 404.
    deleter: Option<Arc<GameDeleter>>,
    /// The settings the web UI may change; without them the settings paths answer 404.
    settings: Option<Arc<dyn Tunables>>,
    /// Restarts the process (a clean stop the service manager follows with a start).
    restart: Option<StopHook>,
    /// The network's DNS server; without it the DNS paths answer 404.
    dns: Option<Arc<DnsLink>>,
    /// What was updated when; without it the history path answers 404.
    history: Option<Arc<BuildHistory>>,
    /// Counters over time; without them the stats path answers 404.
    stats: Option<Arc<CountersKept>>,
    /// The prefill jobs that ended; without them the runs path answers 404.
    runs: Option<Arc<PrefillRuns>>,
    /// The games kept up to date on their own; without it the auto-update path answers 404.
    auto: Option<Arc<lanstash_app::auto_update::AutoUpdate>>,
    /// The checks the service makes of itself; without them the health path answers 404.
    health: Option<HealthOf>,
    /// What the clients are called; without it the clients paths answer 404.
    clients: Option<Arc<ClientNames>>,
    /// The games kept private; without them the hidden paths answer 404.
    hidden: Option<Arc<lanstash_app::hidden::HiddenGames>>,
    /// Each account's private games; without them the private paths answer 404.
    private: Option<Arc<lanstash_app::private_games::PrivateGames>>,
    /// Sign-ins and accounts; without them every request is answered as an admin's ([`access`]).
    sessions: Option<Arc<lanstash_app::sessions::Sessions>>,
    /// Invites to make an account; without them the invite paths answer 404.
    invites: Option<Arc<lanstash_app::invites::Invites>>,
    /// Event streams open now (each holds a connection for as long as its page is open).
    streams: AtomicUsize,
    /// The event streams open now, per address ([`events::PER_ADDRESS`] at most each).
    streams_from: Mutex<std::collections::BTreeMap<std::net::IpAddr, usize>>,
    /// The names of no meaning a guest sees for the clients' addresses.
    pseudonyms: crate::anon::Pseudonyms,
    /// The built web UI's directory ([`web`]); without it `/` is the built-in page.
    web: Option<std::path::PathBuf>,
    /// A newer release, for `GET /api/v1/version`; without it none is said.
    newer: Option<version::Newer>,
}

/// The most event streams open at once: the rest of the admin port's connections stay free (for a stop).
pub const MAX_STREAMS: usize = 8;

impl std::fmt::Debug for AdminApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminApi")
            .field("interval", &self.interval)
            .field("platforms", &self.platforms)
            .finish_non_exhaustive()
    }
}

impl AdminApi {
    /// `interval`: time between two events of a stream.
    #[must_use]
    pub fn new(status: Arc<Status>, interval: Duration) -> Self {
        Self {
            status,
            counters: Vec::new(),
            interval: interval.max(Duration::from_millis(100)),
            last: Mutex::new(None),
            stop: None,
            art: None,
            platforms: Vec::new(),
            deleter: None,
            settings: None,
            restart: None,
            dns: None,
            history: None,
            stats: None,
            runs: None,
            auto: None,
            health: None,
            clients: None,
            hidden: None,
            private: None,
            sessions: None,
            invites: None,
            streams: AtomicUsize::new(0),
            streams_from: Mutex::default(),
            pseudonyms: crate::anon::Pseudonyms::default(),
            web: None,
            newer: None,
        }
    }

    /// Adds the counter group `name` to every snapshot, read from `counters`.
    #[must_use]
    pub fn with_counters(mut self, name: &'static str, counters: Counters) -> Self {
        self.counters.push((name, counters));
        self
    }

    /// The platforms the platform paths name.
    #[must_use]
    pub fn with_platforms(mut self, platforms: Vec<Arc<dyn Platform>>) -> Self {
        self.platforms = platforms;
        self
    }

    /// Enables the art paths.
    #[must_use]
    pub fn with_art(mut self, art: Arc<Art>) -> Self {
        self.art = Some(art);
        self
    }

    /// Enables `DELETE /api/v1/games/<platform>/<game>`.
    #[must_use]
    pub fn with_deleter(mut self, deleter: Arc<GameDeleter>) -> Self {
        self.deleter = Some(deleter);
        self
    }

    /// Enables the shutdown request (from this machine only), running `stop`.
    #[must_use]
    pub fn with_settings(mut self, settings: Arc<dyn Tunables>, restart: StopHook) -> Self {
        self.settings = Some(settings);
        self.restart = Some(restart);
        self
    }

    #[must_use]
    pub fn with_stop(mut self, stop: StopHook) -> Self {
        self.stop = Some(stop);
        self
    }

    fn platform(&self, id: &str) -> Option<&dyn Platform> {
        self.platforms.iter().find(|p| p.id() == id).map(AsRef::as_ref)
    }

    /// Every platform's prefill.
    fn prefills(&self) -> Vec<Arc<dyn Prefill>> {
        self.platforms.iter().filter_map(|p| p.prefill()).collect()
    }

    /// The prefill started last, of any platform, and its platform.
    fn latest_prefill(&self) -> Option<(String, PrefillStatus)> {
        self.platforms
            .iter()
            .filter_map(|p| Some((p.id().to_owned(), p.prefill()?.status()?)))
            .max_by_key(|(_, s)| s.started_ms)
    }

    /// A snapshot (with the further counters) and its rates since `prev`.
    fn snapshot(&self, prev: &mut Option<Sample>) -> String {
        let mut s = self.status.snapshot();
        s.prefill = self.latest_prefill();
        s.prefills = self.last_prefills();
        s.deleting = self.deleter.as_ref().and_then(|d| d.status());
        s.builds_changed =
            self.platforms.iter().filter_map(|p| Some((p.id().to_owned(), p.builds_changed_ms()?))).collect();
        s.clients = self.live_clients();
        for (name, values) in &self.counters {
            s.counters.push(CounterGroup { name, values: values() });
        }
        let (r, sample) = rates(prev.as_ref(), s.now_ms, &s.counters);
        *prev = Some(sample);
        render_json(&s, &r)
    }

    /// `/api/v1/games/<platform>/<game>[/nginx]`.
    fn game(&self, target: &str) -> Answer {
        let view = rest(target, GAMES).split_once('/').and_then(|(id, game)| {
            let p = self.platform(id)?;
            if let Some(game) = game.strip_suffix("/prune").filter(|g| valid_game(g)) {
                return p.prune(game).map(|r| r.map(|(view, _)| view));
            }
            match game.strip_suffix("/nginx") {
                Some(game) if valid_game(game) => p.nginx(game),
                None if valid_game(game) => p.game(game),
                _ => None,
            }
        });
        Answer::view(target, view, "no such game")
    }

    /// `/api/v1/art/<platform>/<id>[/<kind>]`.
    fn art(&self, target: &str) -> Answer {
        let mut parts = rest(target, ART).split('/');
        let (platform, id, kind) = (parts.next(), parts.next(), ArtKind::parse(parts.next()));
        let found = match (&self.art, platform, id, kind, parts.next()) {
            (Some(art), Some(platform), Some(id), Some(kind), None) => art.get(platform, id, kind),
            _ => Err(ArtError::Unknown),
        };
        match found {
            Ok(img) => {
                Answer { cache: "public, max-age=86400", ..Answer::new(200, img.content_type, img.body.clone()) }
            }
            Err(ArtError::Unknown) => Answer::error(target, 404, "not-found", "no art source for this game"),
            Err(ArtError::Missing) => {
                Answer::error(target, 404, "not-found", "the platform has no image for this game")
            }
            Err(ArtError::Upstream(e)) => Answer::error(target, 502, "upstream", &e),
        }
    }

    fn shutdown(&self, target: &str, local: bool) -> Answer {
        match (&self.stop, local) {
            (None, _) => Answer::error(target, 404, "not-found", "not found"),
            (Some(_), false) => Answer::error(target, 403, "forbidden", "only from this machine"),
            (Some(stop), true) => {
                let stop = Arc::clone(stop);
                // Answer first; the hook ends the process.
                std::thread::spawn(move || stop());
                if is_api(target) {
                    Answer::json(202, "{\"stopping\":true}\n".to_owned())
                } else {
                    Answer::text(202, "stopping\n")
                }
            }
        }
    }

    /// The whole answer of `route`; [`Route::Events`] is a stream, sent by `handle` instead.
    fn answer(&self, route: Route, target: &str, conn: &ConnState) -> Answer {
        match route {
            Route::Page | Route::Events => self.page(target),
            // Answered by `handle`, which has the request's headers.
            Route::Session
            | Route::SignIn
            | Route::SignOut
            | Route::Users
            | Route::UserAdd
            | Route::UserChange
            | Route::UserRemove
            | Route::Invites
            | Route::InviteMake
            | Route::InviteAccept
            | Route::InviteRevoke
            | Route::InviteShow
            | Route::Private
            | Route::PrivateSet
            | Route::PrivateUnset => Answer::error(target, 400, "bad-request", "needs the request"),
            Route::Art => self.art(target),
            Route::Account | Route::AccountPost | Route::AccountDelete => self.account(route, target, None),
            Route::PrefillStatus => self.prefill(target, false),
            Route::PrefillStop => self.prefill(target, true),
            Route::PrefillStart => self.prefill_start(target),
            Route::PrefillUnqueue => self.prefill_unqueue(target),
            Route::Settings | Route::SettingsSet | Route::SettingsReset => self.settings(route, target),
            Route::Restart => self.restart(target),
            Route::Dns | Route::DnsApply => self.dns(route, target),
            Route::DnsSetup | Route::DnsForget => self.dns_setup(route, target, None),
            Route::Builds => {
                Answer::view(target, self.platform(rest(target, BUILDS)).and_then(|p| p.builds()), "not found")
            }
            Route::Catalog => {
                Answer::view(target, self.platform(rest(target, CATALOG)).and_then(|p| p.catalog()), "not found")
            }
            Route::Setup => self.setup(target),
            Route::History => self.history(target),
            Route::Stats => self.stats(target),
            Route::StatsTop => self.stats_top(target),
            Route::Health => self.health(target),
            Route::PrefillRuns => self.prefill_runs(target),
            Route::AutoUpdate => self.auto_update(target),
            Route::Clients | Route::ClientLabel | Route::ClientUnlabel => self.clients(route, target),
            Route::Hidden | Route::HiddenSet | Route::HiddenUnset => self.hidden(route, target),
            Route::Game => self.game(target),
            Route::GameDelete => self.delete_game(target),
            Route::GamePrune => self.prune_game(target),
            Route::Version => self.version(),
            Route::Status => {
                let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
                Answer::json(200, self.snapshot(&mut last))
            }
            Route::Shutdown => self.shutdown(target, conn.is_local()),
            Route::NotFound => self.not_found(target),
            Route::Method(allow) => {
                Answer { allow: Some(allow), ..Answer::error(target, 405, "method", &format!("only {allow}")) }
            }
        }
    }
}
