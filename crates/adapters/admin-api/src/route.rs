//! Which part of the admin API a request asks for.

/// What a request asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Page,
    Version,
    Status,
    Events,
    Shutdown,
    /// `/api/v1/art/<platform>/<id>[/<kind>]`: a game's image.
    Art,
    /// `/api/v1/builds/<platform>`.
    Builds,
    /// `/api/v1/catalog/<platform>`.
    Catalog,
    /// `/api/v1/games/<platform>/<game>[/nginx|/prune]`.
    Game,
    /// `DELETE /api/v1/games/<platform>/<game>`: deletes what the stash holds of it.
    GameDelete,
    /// `POST /api/v1/games/<platform>/<game>/prune`: deletes what no current build needs.
    GamePrune,
    /// `GET /api/v1/account/<platform>[/qr.svg]`: the account and its sign-in.
    Account,
    /// `POST /api/v1/account/<platform>/sign-in|sign-out`.
    AccountPost,
    /// `DELETE /api/v1/account/<platform>/sign-in`: cancels a waiting sign-in.
    AccountDelete,
    /// `GET /api/v1/prefill`.
    PrefillStatus,
    /// `POST /api/v1/prefill/stop`.
    PrefillStop,
    /// `POST /api/v1/prefill/<platform>/<game>`.
    PrefillStart,
    /// `GET /api/v1/settings`.
    Settings,
    /// `PUT /api/v1/settings/<key>?value=...`.
    SettingsSet,
    /// `DELETE /api/v1/settings/<key>`.
    SettingsReset,
    /// `POST /api/v1/restart`.
    Restart,
    /// `DELETE /api/v1/prefill/<platform>/<game>`: takes a game out of the queue.
    PrefillUnqueue,
    /// `GET /api/v1/dns`: the DNS server's rewrites against lanstash's names, and coverage.
    Dns,
    /// `POST /api/v1/dns/apply`: adds the missing rewrites, leaves one of each duplicate.
    DnsApply,
    /// `PUT /api/v1/dns/setup?kind=&host=&port=&user=[&answer=][&test=1]`: sets up the DNS server.
    DnsSetup,
    /// `DELETE /api/v1/dns/setup`: forgets the DNS server's setup.
    DnsForget,
    /// `GET /api/v1/setup/<platform>/<os>`: the file that sets up a PC for a platform.
    Setup,
    /// `GET /api/v1/history[?limit=N]`: what was updated when, newest first.
    History,
    /// `GET /api/v1/stats?names=&from=&to=[&step=]`: counters over time.
    Stats,
    /// `GET /api/v1/stats/top?prefix=&from=&to=[&limit=]`: the counters that went up most.
    StatsTop,
    /// `GET /api/v1/health`: the checks the service makes of itself.
    Health,
    /// `GET /api/v1/prefill/runs[?platform=][&limit=]`: the prefill jobs that ended.
    PrefillRuns,
    /// `GET /api/v1/auto-update`: the games kept up to date on their own.
    AutoUpdate,
    /// `GET /api/v1/clients?from=&to=`: what each client got over a range.
    Clients,
    /// `PUT /api/v1/clients/<address>?label=`: names a client.
    ClientLabel,
    /// `DELETE /api/v1/clients/<address>`: takes a client's name away.
    ClientUnlabel,
    /// `GET /api/v1/hidden`: the games kept private.
    Hidden,
    /// `PUT /api/v1/hidden/<platform>/<game>`: keeps a game private.
    HiddenSet,
    /// `DELETE /api/v1/hidden/<platform>/<game>`: shows a game again.
    HiddenUnset,
    /// `GET /api/v1/private`: the games the caller keeps private.
    Private,
    /// `PUT /api/v1/private/<platform>/<game>[?account=]`: keeps a game of one's own private.
    PrivateSet,
    /// `DELETE /api/v1/private/<platform>/<game>[?account=]`: takes that mark away.
    PrivateUnset,
    /// `GET /api/v1/session`: who is signed in.
    Session,
    /// `POST /api/v1/session`: signs in (name and password in headers).
    SignIn,
    /// `DELETE /api/v1/session`: signs out.
    SignOut,
    /// `GET /api/v1/users`: lanstash's accounts.
    Users,
    /// `POST /api/v1/users/<name>?role=`: adds an account (its password in a header).
    UserAdd,
    /// `PUT /api/v1/users/<name>[?role=]`: changes an account's role, or a password (in headers).
    UserChange,
    /// `DELETE /api/v1/users/<name>`: takes an account away.
    UserRemove,
    /// `GET /api/v1/invites`: the invites not yet used.
    Invites,
    /// `POST /api/v1/invites?role=[&link=]`: makes an invite.
    InviteMake,
    /// `POST /api/v1/invites/<token>`: makes an account from an invite (name and password in headers).
    InviteAccept,
    /// `DELETE /api/v1/invites/<id>`: takes an invite back.
    InviteRevoke,
    /// `GET /api/v1/invites/<token>`: what an invite makes (its role, links and name, if chosen).
    InviteShow,
    /// Nothing at that path.
    NotFound,
    /// The path is there, but not for this method; the methods it takes.
    Method(&'static str),
}

/// Prefix of the account paths.
pub const ACCOUNT: &str = "/api/v1/account/";
/// Prefix of the art paths.
pub const ART: &str = "/api/v1/art/";
/// Prefix of the builds paths.
pub const BUILDS: &str = "/api/v1/builds/";
/// Prefix of the catalog paths.
pub const CATALOG: &str = "/api/v1/catalog/";
/// Prefix of one client's path.
pub const CLIENTS: &str = "/api/v1/clients/";
/// Prefix of one invite's path.
pub const INVITES: &str = "/api/v1/invites/";
/// Prefix of one account's path.
pub const USERS: &str = "/api/v1/users/";
/// Prefix of one hidden game's path.
pub const HIDDEN: &str = "/api/v1/hidden/";
/// Prefix of one private game's path.
pub const PRIVATE: &str = "/api/v1/private/";
/// Prefix of the game paths.
pub const GAMES: &str = "/api/v1/games/";
/// Prefix of the prefill paths.
pub const PREFILL: &str = "/api/v1/prefill/";
/// The settings path (and the prefix of one setting's).
pub const SETTINGS: &str = "/api/v1/settings";
/// Prefix of the setup paths.
pub const SETUP: &str = "/api/v1/setup/";

/// Paths (the API path first, then older ones), the methods they take, and the route.
const EXACT: &[(&[&str], &str, Route)] = &[
    (&["/"], "GET, HEAD", Route::Page),
    (&["/api/v1/version"], "GET, HEAD", Route::Version),
    (&["/api/v1/prefill"], "GET, HEAD", Route::PrefillStatus),
    (&["/api/v1/prefill/stop"], "POST", Route::PrefillStop),
    (&["/api/v1/settings"], "GET, HEAD", Route::Settings),
    (&["/api/v1/restart"], "POST", Route::Restart),
    (&["/api/v1/dns"], "GET, HEAD", Route::Dns),
    (&["/api/v1/history"], "GET, HEAD", Route::History),
    (&["/api/v1/stats"], "GET, HEAD", Route::Stats),
    (&["/api/v1/stats/top"], "GET, HEAD", Route::StatsTop),
    (&["/api/v1/health"], "GET, HEAD", Route::Health),
    (&["/api/v1/prefill/runs"], "GET, HEAD", Route::PrefillRuns),
    (&["/api/v1/auto-update"], "GET, HEAD", Route::AutoUpdate),
    (&["/api/v1/clients"], "GET, HEAD", Route::Clients),
    (&["/api/v1/hidden"], "GET, HEAD", Route::Hidden),
    (&["/api/v1/private"], "GET, HEAD", Route::Private),
    (&["/api/v1/session"], "GET, HEAD, POST, DELETE", Route::Session),
    (&["/api/v1/users"], "GET, HEAD", Route::Users),
    (&["/api/v1/invites"], "GET, HEAD, POST", Route::Invites),
    (&["/api/v1/dns/apply"], "POST", Route::DnsApply),
    (&["/api/v1/dns/setup"], "PUT, DELETE", Route::DnsSetup),
    (&["/api/v1/status", "/status.json"], "GET, HEAD", Route::Status),
    (&["/api/v1/events", "/events"], "GET", Route::Events),
    (&["/api/v1/shutdown", "/shutdown"], "POST", Route::Shutdown),
];

/// Path prefixes (after the exact paths), the methods they take, and the route.
const PREFIXED: &[(&str, &str, Route)] = &[
    (ACCOUNT, "GET, HEAD, POST, DELETE", Route::Account),
    (ART, "GET, HEAD", Route::Art),
    (BUILDS, "GET, HEAD", Route::Builds),
    (CATALOG, "GET, HEAD", Route::Catalog),
    (CLIENTS, "PUT, DELETE", Route::ClientLabel),
    (GAMES, "GET, HEAD, DELETE, POST", Route::Game),
    (HIDDEN, "PUT, DELETE", Route::HiddenSet),
    (PRIVATE, "PUT, DELETE", Route::PrivateSet),
    (USERS, "POST, PUT, DELETE", Route::UserAdd),
    (INVITES, "GET, HEAD, POST, DELETE", Route::InviteAccept),
    (PREFILL, "POST, DELETE", Route::PrefillStart),
    (SETUP, "GET, HEAD", Route::Setup),
    ("/api/v1/settings/", "PUT, DELETE", Route::Settings),
];

/// Routes a method turns into another: (route, method, the route it becomes).
const BY_METHOD: &[(Route, &str, Route)] = &[
    (Route::Game, "DELETE", Route::GameDelete),
    (Route::Game, "POST", Route::GamePrune),
    (Route::Account, "POST", Route::AccountPost),
    (Route::Account, "DELETE", Route::AccountDelete),
    (Route::PrefillStart, "DELETE", Route::PrefillUnqueue),
    (Route::Settings, "PUT", Route::SettingsSet),
    (Route::Settings, "DELETE", Route::SettingsReset),
    (Route::DnsSetup, "DELETE", Route::DnsForget),
    (Route::ClientLabel, "DELETE", Route::ClientUnlabel),
    (Route::HiddenSet, "DELETE", Route::HiddenUnset),
    (Route::PrivateSet, "DELETE", Route::PrivateUnset),
    (Route::Session, "POST", Route::SignIn),
    (Route::Session, "DELETE", Route::SignOut),
    (Route::UserAdd, "PUT", Route::UserChange),
    (Route::UserAdd, "DELETE", Route::UserRemove),
    (Route::Invites, "POST", Route::InviteMake),
    (Route::InviteAccept, "DELETE", Route::InviteRevoke),
    (Route::InviteAccept, "GET", Route::InviteShow),
    (Route::InviteAccept, "HEAD", Route::InviteShow),
];

/// The path of a target, without its query.
#[must_use]
pub fn path(target: &str) -> &str {
    target.split('?').next().unwrap_or("")
}

/// The part of `target`'s path after `prefix`.
#[must_use]
pub fn rest<'a>(target: &'a str, prefix: &str) -> &'a str {
    path(target).strip_prefix(prefix).unwrap_or("")
}

/// The route of a request (the query is ignored).
#[must_use]
pub fn route(method: &str, target: &str) -> Route {
    let path = path(target);
    let allowed = |allow: &'static str, r: Route| {
        if allow.split(", ").any(|m| m == method) { by_method(r, method) } else { Route::Method(allow) }
    };
    if let Some((_, allow, r)) = EXACT.iter().find(|(paths, _, _)| paths.contains(&path)) {
        return allowed(allow, *r);
    }
    match PREFIXED.iter().find(|(prefix, _, _)| path.starts_with(prefix)) {
        Some((_, allow, r)) => allowed(allow, *r),
        None => Route::NotFound,
    }
}

/// `r`, or the route `method` turns it into.
fn by_method(r: Route, method: &str) -> Route {
    BY_METHOD.iter().find(|(from, m, _)| *from == r && *m == method).map_or(r, |(_, _, to)| *to)
}

/// Whether a path is part of the API (and so answers errors in JSON).
#[must_use]
pub fn is_api(target: &str) -> bool {
    target.starts_with("/api/")
}

/// A game a path may name: one safe path segment (a Battle.net CDN
/// directory, a Steam app id, ...).
#[must_use]
pub fn valid_game(game: &str) -> bool {
    (1..=64).contains(&game.len()) && game.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
