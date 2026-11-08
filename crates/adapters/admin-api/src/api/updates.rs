//! Automatic updates through the API: `GET /api/v1/auto-update`: the games kept up to date on
//! their own, as `{"followed":["<platform>/<game>",..]}` (404 when `auto_update` is off).

use std::sync::Arc;

use lanstash_app::auto_update::AutoUpdate;
use lanstash_domain::json::string;

use super::{AdminApi, Answer};

impl AdminApi {
    /// Enables `/api/v1/auto-update`.
    #[must_use]
    pub fn with_auto_update(mut self, auto: Arc<AutoUpdate>) -> Self {
        self.auto = Some(auto);
        self
    }

    /// `/api/v1/auto-update`.
    pub(super) fn auto_update(&self, target: &str) -> Answer {
        let Some(auto) = &self.auto else {
            return Answer::error(target, 404, "not-found", "automatic updates are off (auto_update)");
        };
        let games: Vec<String> = auto.followed().iter().map(|g| string(g)).collect();
        Answer::json(200, format!("{{\"followed\":[{}]}}\n", games.join(",")))
    }
}
