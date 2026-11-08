//! Each account's private games, through the API:
//! - `GET /api/v1/private`: `{"private":["<platform>/<game>",..]}`, the caller's own;
//! - `PUT /api/v1/private/<platform>/<game>`: keeps a game of the caller's own private;
//! - `DELETE /api/v1/private/<platform>/<game>`: takes that mark away.
//!
//! A player marks only its own games (a platform account linked to it owns them); whoever may hide
//! games (an admin) marks any game, and for another account with `?account=<name>`. Each answers
//! with the caller's (or that account's) list as it is then; 404 without the marks.
//!
//! For everyone without `Hide`, a private game is masked as hidden ones are, by
//! [`lanstash_domain::private::masked_for`]: when every owner marked it, for those who did not.

use std::sync::Arc;

use lanstash_app::private_games::PrivateGames;
use lanstash_domain::accounts::Permission;
use lanstash_domain::json::string;
use lanstash_domain::private::masked_for;

use super::access::Caller;
use super::{AdminApi, Answer};
use crate::route::{PRIVATE, Route, rest};

/// `?account=<name>` of a target.
fn account_asked(target: &str) -> Option<&str> {
    let query = target.split_once('?').map(|(_, q)| q)?;
    query.split('&').find_map(|kv| kv.strip_prefix("account=")).filter(|a| !a.is_empty())
}

impl AdminApi {
    /// Enables `/api/v1/private` and the masking of private games.
    #[must_use]
    pub fn with_private(mut self, private: Arc<PrivateGames>) -> Self {
        self.private = Some(private);
        self
    }

    /// The private games `reader` (an account; none: signed out) sees masked.
    pub(super) fn private_masked(&self, reader: Option<&str>) -> Vec<String> {
        let Some(private) = &self.private else { return Vec::new() };
        let marks = private.marks();
        if marks.is_empty() {
            return Vec::new();
        }
        // The owners of the marked games' platforms, read once.
        let mut owners = std::collections::BTreeMap::new();
        for game in marks.keys() {
            if let Some((platform, _)) = game.split_once('/') {
                owners.entry(platform.to_owned()).or_insert_with(|| self.owners_on(platform));
            }
        }
        let owners_of = |game: &str| {
            let (platform, id) = game.split_once('/').unwrap_or((game, ""));
            owners.get(platform).and_then(|o| o.get(id)).cloned().unwrap_or_default()
        };
        let links = |account: &str| self.sessions.as_ref().map(|s| s.accounts().links(account)).unwrap_or_default();
        masked_for(reader, &marks, &owners_of, &links)
    }

    /// `/api/v1/private` and `/api/v1/private/<platform>/<game>`.
    pub(super) fn private(&self, route: Route, target: &str, caller: &Caller) -> Answer {
        let Some(private) = &self.private else {
            return Answer::error(target, 404, "not-found", "no games are kept private");
        };
        let me = match caller {
            Caller::Signed { name, .. } => Some(name.as_str()),
            _ => None,
        };
        let admin = caller.can(Permission::Hide);
        // Whose marks: one's own; an admin may name another account.
        let Some(account) = account_asked(target).filter(|_| admin).or(me) else {
            return Answer::error(target, 401, "sign-in", "sign in to keep games private");
        };
        let on = match route {
            Route::PrivateSet => Some(true),
            Route::PrivateUnset => Some(false),
            _ => None,
        };
        if let Some(on) = on {
            let game = rest(target, PRIVATE);
            let (platform, id) = game.split_once('/').unwrap_or((game, ""));
            if on && !admin && !self.owns(caller, platform, id) {
                return Answer::error(target, 403, "not-yours", "only a game of one's own can be kept private");
            }
            if let Err(e) = private.set(account, game, on) {
                return Answer::error(target, 400, "bad-request", &e);
            }
        }
        let games: Vec<String> = private.of(account).iter().map(|g| string(g)).collect();
        Answer::json(200, format!("{{\"private\":[{}]}}\n", games.join(",")))
    }
}
