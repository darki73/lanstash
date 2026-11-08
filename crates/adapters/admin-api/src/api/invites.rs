//! Invites through the API (permission `accounts`, but for the two its link opens):
//! - `GET /api/v1/invites`: `{"invites":[{"id","role","links","until_ms","name"}]}` (never a token;
//!   `name` null unless the admin chose the account's name);
//! - `POST /api/v1/invites?role=<role>[&link=<platform>/<account>]...[&name=]`: `{"token","until_ms"}`;
//! - `DELETE /api/v1/invites/<id>`: takes one back;
//! - open to anyone holding the link (a wrong token makes the address wait as a wrong sign-in does):
//!   `GET /api/v1/invites/<token>`: `{"role","links","until_ms","name"}`; `POST` with the name and
//!   password in headers: makes the account with the invite's role and links, and signs it in (the
//!   session cookie).

use std::sync::Arc;

use lanstash_app::invites::{AcceptError, Invite, Invites};
use lanstash_app::serve::ConnState;
use lanstash_app::sessions::LIFE_MS;
use lanstash_domain::accounts::Role;
use lanstash_domain::http::RequestHead;
use lanstash_domain::json::string;

use super::settings::decode;
use super::users::{PASSWORD_HEADER, USER_HEADER, cookie};
use super::{AdminApi, Answer};
use crate::route::{INVITES, Route, rest};

/// The values of `key` in `target`'s query, decoded.
fn params(target: &str, key: &str) -> Vec<String> {
    let Some((_, q)) = target.split_once('?') else { return Vec::new() };
    q.split('&').filter_map(|kv| kv.strip_prefix(key)?.strip_prefix('=').and_then(decode)).collect()
}

impl AdminApi {
    /// Enables `/api/v1/invites`.
    #[must_use]
    pub fn with_invites(mut self, invites: Arc<Invites>) -> Self {
        self.invites = Some(invites);
        self
    }

    /// `/api/v1/invites` and `/api/v1/invites/<token or id>`.
    pub(super) fn invites(&self, route: Route, req: &RequestHead, conn: &ConnState) -> Answer {
        let target = &req.target;
        let (Some(invites), Some(sessions)) = (&self.invites, &self.sessions) else {
            return Answer::error(target, 404, "not-found", "lanstash has no accounts");
        };
        match route {
            Route::InviteMake => {
                let role = match params(target, "role").first() {
                    None => Role::Viewer,
                    Some(r) => match Role::parse(r) {
                        Some(role) => role,
                        None => return Answer::error(target, 400, "bad-request", &format!("no role called {r:?}")),
                    },
                };
                let name = params(target, "name").into_iter().next().filter(|n| !n.is_empty());
                match invites.make(role, &params(target, "link"), name.as_deref()) {
                    Ok((token, until)) => {
                        Answer::json(200, format!("{{\"token\":{},\"until_ms\":{until}}}\n", string(&token)))
                    }
                    Err(e) => Answer::error(target, 400, "bad-request", &e),
                }
            }
            Route::InviteRevoke => match invites.revoke(rest(target, INVITES)) {
                Ok(()) => Answer::json(200, "{\"ok\":true}\n".into()),
                Err(e) => Answer::error(target, 404, "not-found", &e),
            },
            Route::InviteShow | Route::InviteAccept if let Some(ms) = sessions.waiting(&from(conn)) => Answer::json(
                429,
                format!("{{\"error\":\"wait {} s\",\"kind\":\"wait\",\"retry_ms\":{ms}}}\n", ms.div_ceil(1000)),
            ),
            Route::InviteShow => match invites.show(rest(target, INVITES)) {
                Some(i) => Answer::json(200, format!("{}\n", invite_json(&i, false))),
                None => {
                    sessions.failed(&from(conn));
                    Answer::error(target, 404, "not-found", &AcceptError::Unknown.to_string())
                }
            },
            Route::InviteAccept => {
                let from = from(conn);
                let (Some(name), Some(password)) =
                    (req.header(USER_HEADER).and_then(decode), req.header(PASSWORD_HEADER).and_then(decode))
                else {
                    return Answer::error(target, 400, "bad-request", "a name and a password are needed");
                };
                match invites.accept(rest(target, INVITES), &name, &password) {
                    Ok(role) => match sessions.start(&name) {
                        Ok(token) => Answer {
                            cookie: Some(cookie(&token, LIFE_MS / 1000, conn.is_secure())),
                            ..Answer::json(
                                200,
                                format!("{{\"name\":{},\"role\":\"{}\"}}\n", string(&name), role.name()),
                            )
                        },
                        Err(e) => Answer::error(target, 500, "internal", &e),
                    },
                    Err(AcceptError::Unknown) => {
                        sessions.failed(&from);
                        Answer::error(target, 404, "not-found", &AcceptError::Unknown.to_string())
                    }
                    Err(AcceptError::Refused(why)) => Answer::error(target, 400, "bad-request", &why),
                }
            }
            _ => {
                let all: Vec<String> = invites.list().iter().map(|i| invite_json(i, true)).collect();
                Answer::json(200, format!("{{\"invites\":[{}]}}\n", all.join(",")))
            }
        }
    }
}

/// The address a request comes from, as the sign-in wait knows it.
fn from(conn: &ConnState) -> String {
    conn.client().map_or_else(|| "unknown".to_string(), |ip| ip.to_string())
}

/// An invite in JSON: its role, links, end and chosen name, and its id for an admin's list.
fn invite_json(i: &Invite, with_id: bool) -> String {
    let links: Vec<String> = i.links.iter().map(|l| string(l)).collect();
    let id = if with_id { format!("\"id\":{},", string(&i.id)) } else { String::new() };
    let name = i.name.as_deref().map_or_else(|| "null".to_string(), string);
    format!(
        "{{{id}\"role\":\"{}\",\"links\":[{}],\"until_ms\":{},\"name\":{name}}}",
        i.role.name(),
        links.join(","),
        i.until_ms
    )
}
