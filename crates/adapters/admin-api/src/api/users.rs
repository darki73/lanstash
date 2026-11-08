//! Signing in and lanstash's accounts, through the API. Names and passwords come in headers
//! (percent-encoded), never in the path, where logs and the browser's history would keep them.
//! - `GET /api/v1/session`: `{"name","role","accounts"}`: who asks (`name` null and `role`
//!   `admin` from this machine, `guest` for no one signed in), and whether lanstash has accounts;
//! - `POST /api/v1/session` with [`USER_HEADER`] and [`PASSWORD_HEADER`]: signs in, the session in
//!   an `HttpOnly`, `SameSite=Strict` cookie (401 wrong, 429 wait first, with `retry_ms`);
//! - `DELETE /api/v1/session`: signs out;
//! - `GET /api/v1/users`: `{"users":[{"name","role"}]}` (admin);
//! - `POST /api/v1/users/<name>[?role=admin|viewer]` with the password: adds one (admin; a viewer
//!   unless said);
//! - `PUT /api/v1/users/<name>?role=`: changes its role (admin); `PUT /api/v1/users/<name>` with a
//!   new password: changes it (an admin, or the account itself with [`OLD_PASSWORD_HEADER`]); its
//!   sessions end;
//! - `DELETE /api/v1/users/<name>`: takes it away (admin), and its sessions.

use std::sync::Arc;

use lanstash_app::serve::ConnState;
use lanstash_app::sessions::{LIFE_MS, Sessions, SignInError};
use lanstash_domain::accounts::{Permission, Role};
use lanstash_domain::http::RequestHead;
use lanstash_domain::json::string;

use super::access::{Caller, SESSION_COOKIE, session_token};
use super::settings::decode;
use super::{AdminApi, Answer};
use crate::route::{Route, USERS, rest};

/// The account name of a sign-in.
pub const USER_HEADER: &str = "x-lanstash-user";
/// A password: the sign-in's, or a new one.
pub const PASSWORD_HEADER: &str = "x-lanstash-password";
/// The password being changed, when an account changes its own.
pub const OLD_PASSWORD_HEADER: &str = "x-lanstash-old-password";

/// A header's value, percent-decoded.
fn header(req: &RequestHead, name: &str) -> Option<String> {
    req.header(name).and_then(decode)
}

/// The value of `role` in `target`'s query.
fn role_param(target: &str) -> Result<Option<Role>, String> {
    let Some((_, q)) = target.split_once('?') else { return Ok(None) };
    match q.split('&').find_map(|kv| kv.strip_prefix("role=")) {
        None => Ok(None),
        Some(r) => Role::parse(r).map(Some).ok_or_else(|| format!("no role called {r:?}: admin, player or viewer")),
    }
}

/// The session cookie; over HTTPS it is never sent over plain HTTP (`Secure`).
pub(super) fn cookie(token: &str, max_age_s: u64, secure: bool) -> String {
    let s = if secure { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age_s}{s}")
}

const fn ok() -> &'static str {
    "{\"ok\":true}\n"
}

impl AdminApi {
    /// Turns accounts on: sign-in, sessions and who may do what.
    #[must_use]
    pub fn with_sessions(mut self, sessions: Arc<Sessions>) -> Self {
        self.sessions = Some(sessions);
        self
    }

    /// `/api/v1/session`.
    pub(super) fn session(&self, route: Route, req: &RequestHead, conn: &ConnState, caller: &Caller) -> Answer {
        let target = &req.target;
        let Some(sessions) = &self.sessions else {
            return match route {
                Route::Session => Answer::json(200, "{\"name\":null,\"role\":\"admin\",\"accounts\":false}\n".into()),
                _ => Answer::error(target, 404, "not-found", "lanstash has no accounts"),
            };
        };
        match route {
            Route::SignIn => {
                let (Some(name), Some(password)) = (header(req, USER_HEADER), header(req, PASSWORD_HEADER)) else {
                    return Answer::error(target, 400, "bad-request", "a name and a password are needed");
                };
                let from = conn.client().map_or_else(|| "unknown".to_string(), |ip| ip.to_string());
                match sessions.sign_in(&from, &name, &password) {
                    Ok((token, role)) => Answer {
                        cookie: Some(cookie(&token, LIFE_MS / 1000, conn.is_secure())),
                        ..Answer::json(200, format!("{{\"name\":{},\"role\":\"{}\"}}\n", string(&name), role.name()))
                    },
                    Err(SignInError::Wrong) => Answer::error(target, 401, "wrong", "wrong name or password"),
                    Err(SignInError::Wait(ms)) => Answer::json(
                        429,
                        format!(
                            "{{\"error\":\"too many wrong sign-ins: wait {} s\",\"kind\":\"wait\",\"retry_ms\":{ms}}}\n",
                            ms.div_ceil(1000)
                        ),
                    ),
                    Err(SignInError::NotKept(e)) => Answer::error(target, 500, "internal", &e),
                }
            }
            Route::SignOut => {
                if let Some(t) = session_token(req) {
                    sessions.sign_out(t);
                }
                Answer { cookie: Some(cookie("", 0, conn.is_secure())), ..Answer::json(200, ok().into()) }
            }
            _ => {
                let has = sessions.accounts().has_admin();
                let (name, role) = match caller {
                    Caller::Local => ("null".to_string(), "admin"),
                    Caller::Signed { name, role } => (string(name), role.name()),
                    Caller::Guest => ("null".to_string(), "guest"),
                };
                let can: Vec<String> = caller.permissions().iter().map(|p| string(p.name())).collect();
                Answer::json(
                    200,
                    format!(
                        "{{\"name\":{name},\"role\":\"{role}\",\"accounts\":true,\"admin\":{has},\"permissions\":[{}],\"links\":[{}]}}\n",
                        can.join(","),
                        self.links_of(caller).iter().map(|l| string(l)).collect::<Vec<_>>().join(",")
                    ),
                )
            }
        }
    }

    /// `/api/v1/users` and `/api/v1/users/<name>`.
    pub(super) fn users(&self, route: Route, req: &RequestHead, caller: &Caller) -> Answer {
        let target = &req.target;
        let Some(sessions) = &self.sessions else {
            return Answer::error(target, 404, "not-found", "lanstash has no accounts");
        };
        let accounts = sessions.accounts();
        let name = decode(rest(target, USERS)).unwrap_or_default();
        let done = |r: Result<(), String>| match r {
            Ok(()) => Answer::json(200, ok().into()),
            Err(e) => Answer::error(target, 400, "bad-request", &e),
        };
        let role = match role_param(target) {
            Ok(r) => r,
            Err(e) => return Answer::error(target, 400, "bad-request", &e),
        };
        match route {
            Route::UserAdd => {
                let Some(password) = header(req, PASSWORD_HEADER) else {
                    return Answer::error(target, 400, "bad-request", "a password is needed");
                };
                done(accounts.add(&name, role.unwrap_or(Role::Viewer), &password))
            }
            Route::UserChange => {
                let link = |key: &str| {
                    target.split_once('?').and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix(key)))
                };
                if let Some((l, on)) = link("link=").map(|l| (l, true)).or_else(|| link("unlink=").map(|l| (l, false)))
                {
                    if !caller.can(Permission::Accounts) {
                        return Answer::error(target, 403, "admin-only", "only an admin may link accounts");
                    }
                    return done(accounts.set_link(&name, &decode(l).unwrap_or_default(), on));
                }
                if let Some(role) = role {
                    if !caller.can(Permission::Accounts) {
                        return Answer::error(target, 403, "admin-only", "only an admin may change a role");
                    }
                    return done(accounts.set_role(&name, role));
                }
                let Some(password) = header(req, PASSWORD_HEADER) else {
                    return Answer::error(target, 400, "bad-request", "a role or a new password is needed");
                };
                let own = matches!(caller, Caller::Signed { name: n, .. } if *n == name);
                if !caller.can(Permission::Accounts) {
                    let old = header(req, OLD_PASSWORD_HEADER).unwrap_or_default();
                    if !own || accounts.check(&name, &old).is_none() {
                        return Answer::error(
                            target,
                            403,
                            "admin-only",
                            "only an admin, or the account with its password",
                        );
                    }
                }
                let r = accounts.set_password(&name, &password);
                if r.is_ok() {
                    sessions.forget(&name);
                }
                done(r)
            }
            Route::UserRemove => {
                let r = accounts.remove(&name);
                if r.is_ok() {
                    sessions.forget(&name);
                    // Its private games are its no more (best effort: the account is gone either way).
                    if let Some(p) = &self.private {
                        let _ = p.forget(&name);
                    }
                }
                done(r)
            }
            _ => {
                let all: Vec<String> = accounts
                    .list()
                    .iter()
                    .map(|(n, r)| {
                        let links: Vec<String> = accounts.links(n).iter().map(|l| string(l)).collect();
                        format!("{{\"name\":{},\"role\":\"{}\",\"links\":[{}]}}", string(n), r.name(), links.join(","))
                    })
                    .collect();
                Answer::json(200, format!("{{\"users\":[{}]}}\n", all.join(",")))
            }
        }
    }
}
