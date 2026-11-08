//! The games kept private, through the API:
//! - `GET /api/v1/hidden`: `{"hidden":["<platform>/<game>",..]}`;
//! - `PUT /api/v1/hidden/<platform>/<game>`: marks the game hidden;
//! - `DELETE /api/v1/hidden/<platform>/<game>`: takes the mark away.
//!
//! Each answers with the list as it is then (404 without the list).

use std::sync::Arc;

use lanstash_app::hidden::HiddenGames;
use lanstash_domain::json::string;

use super::{AdminApi, Answer};
use crate::route::{HIDDEN, Route, rest};

impl AdminApi {
    /// Enables `/api/v1/hidden`.
    #[must_use]
    pub fn with_hidden(mut self, hidden: Arc<HiddenGames>) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// `/api/v1/hidden` and `/api/v1/hidden/<platform>/<game>`.
    pub(super) fn hidden(&self, route: Route, target: &str) -> Answer {
        let Some(hidden) = &self.hidden else {
            return Answer::error(target, 404, "not-found", "no games are kept private");
        };
        let change = match route {
            Route::HiddenSet => Some(true),
            Route::HiddenUnset => Some(false),
            _ => None,
        };
        if let Some(on) = change
            && let Err(e) = hidden.set(rest(target, HIDDEN), on)
        {
            return Answer::error(target, 400, "bad-request", &e);
        }
        let games: Vec<String> = hidden.list().iter().map(|g| string(g)).collect();
        Answer::json(200, format!("{{\"hidden\":[{}]}}\n", games.join(",")))
    }
}
