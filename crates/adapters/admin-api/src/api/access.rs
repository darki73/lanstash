//! Who may do what through the admin API, and the request's way in. With accounts on (sessions
//! given), a request is from:
//! - this machine (loopback): an admin, so the command line (`stop`, `prefill`, `doctor`) needs no
//!   sign-in;
//! - a signed-in account (its session cookie): its role's permissions as the accounts say now;
//! - anyone else: a guest, who holds none.
//!
//! Reading is open to all but for the DNS server's state (`dns`) and who the clients are (any
//! signed-in account, [`names_clients`]); a route that changes something needs the permission
//! [`needs`] names (a guest is asked to sign in, 401; an account without it is refused, 403).
//! Without accounts (no sessions given) every request is answered as before. The guard (other
//! sites, outside host names) comes first either way.
//!
//! For anyone without `Hide`, every JSON answer and every status event has the hidden games masked
//! ([`crate::mask`]), and a hidden game's own page and art are not found. For a guest, every JSON
//! answer and status event leaves out the clients' addresses and names ([`crate::anon`]).

use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_domain::accounts::{Permission, Role};
use lanstash_domain::http::{RequestHead, reason_phrase};

use super::dns::PASSWORD_HEADER;
use super::events::StreamView;
use super::{AdminApi, Answer, account};
use crate::mask::Mask;
use crate::route::{ART, BUILDS, CATALOG, GAMES, Route, rest, route};

/// The cookie a session is kept in.
pub const SESSION_COOKIE: &str = "lanstash_session";

/// The permission a route needs; none: anyone may (reading).
#[must_use]
pub fn needs(route: Route) -> Option<Permission> {
    Some(match route {
        Route::PrefillStart | Route::PrefillStop | Route::PrefillUnqueue => Permission::Prefill,
        Route::SettingsSet | Route::SettingsReset | Route::Restart => Permission::Settings,
        // Reading the DNS server's state names its address and admin user, and has lanstash ask it.
        Route::Dns | Route::DnsApply | Route::DnsSetup | Route::DnsForget => Permission::Dns,
        Route::GameDelete | Route::GamePrune => Permission::Delete,
        Route::Account | Route::AccountPost | Route::AccountDelete => Permission::Platforms,
        Route::ClientLabel | Route::ClientUnlabel => Permission::Clients,
        Route::Hidden | Route::HiddenSet | Route::HiddenUnset => Permission::Hide,
        Route::Users | Route::UserAdd | Route::UserRemove => Permission::Accounts,
        Route::Invites | Route::InviteMake | Route::InviteRevoke => Permission::Accounts,
        _ => return None,
    })
}

/// Who is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// This machine, or any request while lanstash has no accounts.
    Local,
    /// A signed-in account and its role now.
    Signed {
        name: String,
        role: Role,
    },
    Guest,
}

impl Caller {
    /// Whether the caller holds `p`.
    #[must_use]
    pub fn can(&self, p: Permission) -> bool {
        match self {
            Self::Local => true,
            Self::Signed { role, .. } => role.can(p),
            Self::Guest => false,
        }
    }

    /// The account's name, when it is one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Signed { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The caller's permissions.
    #[must_use]
    pub fn permissions(&self) -> Vec<Permission> {
        Permission::ALL.into_iter().filter(|p| self.can(*p)).collect()
    }
}

/// The session token a request's cookies hold.
#[must_use]
pub fn session_token(req: &RequestHead) -> Option<&str> {
    req.header("cookie")?
        .split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find_map(|(k, v)| (k == SESSION_COOKIE && !v.is_empty()).then_some(v))
}

impl AdminApi {
    /// What `reader` (an account; none: signed out), without `Hide`, gets masked: the hidden games,
    /// the private games it may not see, and always the account names (`owners`), so a mask even
    /// with nothing hidden.
    pub(super) fn viewer_mask(&self, reader: Option<&str>) -> Option<Mask> {
        let mut hidden = self.hidden.as_ref().map(|h| h.list()).unwrap_or_default();
        for g in self.private_masked(reader) {
            if !hidden.contains(&g) {
                hidden.push(g);
            }
        }
        if hidden.is_empty() {
            return Some(Mask::default());
        }
        let dirs: Vec<_> =
            self.status.usage_now().into_iter().map(|u| (u.game.service, u.game.game, u.group)).collect();
        Some(Mask::new(&hidden, &dirs))
    }

    /// Who `req` comes from.
    pub(super) fn caller(&self, req: &RequestHead, conn: &ConnState) -> Caller {
        let Some(sessions) = &self.sessions else { return Caller::Local };
        if conn.is_local() {
            return Caller::Local;
        }
        match session_token(req).and_then(|t| sessions.who(t)) {
            Some((name, role)) => Caller::Signed { name, role },
            None => Caller::Guest,
        }
    }
}

/// The platform a page is about (a platform's builds or catalog, a game's page or art), and whether
/// its list leaves hidden games out.
fn page_platform(route: Route, target: &str) -> (Option<&str>, bool) {
    let first = |prefix| rest(target, prefix).split('/').next().filter(|p| !p.is_empty());
    match route {
        Route::Builds => (first(BUILDS), true),
        Route::Catalog => (first(CATALOG), true),
        Route::Game => (first(GAMES), false),
        _ => (None, false),
    }
}

/// Whether `target` is a hidden game's own page or art.
fn hidden_page(mask: &Mask, route: Route, target: &str) -> bool {
    let prefix = match route {
        Route::Game => GAMES,
        Route::Art => ART,
        _ => return false,
    };
    let mut parts = rest(target, prefix).split('/');
    matches!((parts.next(), parts.next()), (Some(p), Some(g)) if mask.token(p, g).is_some())
}

/// Whether `target` asks who the clients are: the clients' page, or counters of one client
/// (`client/<address>/...`, as a name or a prefix), which only a signed-in account may read.
#[must_use]
pub fn names_clients(route: Route, target: &str) -> bool {
    let query = target.split_once('?').map_or("", |(_, q)| q).to_ascii_lowercase();
    route == Route::Clients
        || (matches!(route, Route::Stats | Route::StatsTop)
            && (query.contains("client/") || query.contains("client%2f")))
}

/// What `caller` may see of the status.
fn view_of(caller: &Caller) -> StreamView {
    StreamView { masked: !caller.can(Permission::Hide), guest: *caller == Caller::Guest }
}

/// Why `caller` may not take `route`, as an answer; none when it may.
fn refused(route: Route, caller: &Caller, target: &str) -> Option<Answer> {
    if *caller == Caller::Guest && names_clients(route, target) {
        return Some(Answer::error(target, 401, "sign-in", "sign in to see who the clients are"));
    }
    let p = needs(route)?;
    if caller.can(p) {
        return None;
    }
    Some(match caller {
        Caller::Guest => Answer::error(target, 401, "sign-in", "sign in to do this"),
        _ => Answer::error(target, 403, "admin-only", &format!("this needs the permission {}", p.name())),
    })
}

impl RequestHandler for AdminApi {
    fn handle(&self, req: &RequestHead, conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        let r = route(&req.method, &req.target);
        let caller = self.caller(req, conn);
        let sees_hidden = caller.can(Permission::Hide);
        let mask = if sees_hidden { None } else { self.viewer_mask(caller.name()) };
        let mut a = match r {
            _ if let Some(why) = crate::guard::refused(req) => Answer::error(&req.target, 403, "forbidden", why),
            // A player may start a prefill of a game its linked accounts own.
            Route::PrefillStart
                if !caller.can(Permission::Prefill)
                    && caller.can(Permission::PrefillOwn)
                    && self.owns_prefill(&caller, &req.target) =>
            {
                self.answer(r, &req.target, conn)
            }
            _ if let Some(no) = refused(r, &caller, &req.target) => no,
            _ if mask.as_ref().is_some_and(|m| hidden_page(m, r, &req.target)) => {
                Answer::error(&req.target, 404, "not-found", "not found")
            }
            Route::Events => {
                let from = if conn.is_local() { None } else { conn.client() };
                let still = || self.caller(req, conn) == caller;
                match self.stream(sink, (view_of(&caller), caller.name()), from, &still) {
                    Some(r) => return r,
                    None => Answer::error(&req.target, 503, "busy", "too many pages follow the status; close one"),
                }
            }
            // Passwords and codes come in headers, never in the path.
            r @ (Route::DnsSetup | Route::DnsForget) => self.dns_setup(r, &req.target, req.header(PASSWORD_HEADER)),
            r @ Route::AccountPost => self.account(r, &req.target, req.header(account::SECRET_HEADER)),
            r @ (Route::Session | Route::SignIn | Route::SignOut) => self.session(r, req, conn, &caller),
            r @ (Route::Private | Route::PrivateSet | Route::PrivateUnset) => self.private(r, &req.target, &caller),
            r @ (Route::Users | Route::UserAdd | Route::UserChange | Route::UserRemove) => self.users(r, req, &caller),
            r
            @ (Route::Invites | Route::InviteMake | Route::InviteAccept | Route::InviteRevoke | Route::InviteShow) => {
                self.invites(r, req, conn)
            }
            r => self.answer(r, &req.target, conn),
        };
        // Someone who prefills only its own games sees only those in a catalog.
        if r == Route::Catalog
            && a.status == 200
            && !caller.can(Permission::Prefill)
            && caller.can(Permission::PrefillOwn)
            && let (Some(platform), _) = page_platform(r, &req.target)
        {
            a.body = self.catalog_for(&caller, platform, &String::from_utf8_lossy(&a.body)).into_bytes();
        }
        if let Some(m) = &mask
            && a.ctype == "application/json"
        {
            let (platform, drop) = page_platform(r, &req.target);
            a.body = m.json(&String::from_utf8_lossy(&a.body), platform, drop).into_bytes();
        }
        // A guest sees what the clients download, never who they are.
        if caller == Caller::Guest && a.ctype == "application/json" {
            a.body = crate::anon::without_clients(&String::from_utf8_lossy(&a.body), &self.pseudonyms).into_bytes();
        }
        let mut headers = vec![("Content-Type", a.ctype.as_str()), ("Cache-Control", a.cache)];
        if a.status == 200 && a.cache != "no-store" {
            headers.push(("X-Content-Type-Options", "nosniff"));
        }
        if let Some(allow) = a.allow {
            headers.push(("Allow", allow));
        }
        if let Some(d) = &a.disposition {
            headers.push(("Content-Disposition", d.as_str()));
            headers.push(("X-Content-Type-Options", "nosniff"));
        }
        if let Some(c) = &a.cookie {
            headers.push(("Set-Cookie", c.as_str()));
        }
        let head = ResponseHead {
            status: a.status,
            reason: reason_phrase(a.status),
            headers: &headers,
            content_length: a.body.len() as u64,
        };
        sink.send_head(&head)?;
        // The server drops the body of a HEAD answer.
        sink.send_body(&a.body)
    }
}
