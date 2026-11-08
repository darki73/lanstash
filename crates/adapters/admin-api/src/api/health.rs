//! The health endpoint: `GET /api/v1/health`, the checks the service makes of
//! itself (`{"checks":[{"level","what","detail"}]}`, [`lanstash_app::health`]),
//! made when asked. 404 without them.

use std::sync::Arc;

use lanstash_app::health::{Check, json};

use super::{AdminApi, Answer};

/// The service's checks, made when called.
pub type HealthOf = Arc<dyn Fn() -> Vec<Check> + Send + Sync>;

impl AdminApi {
    /// Enables the health path.
    #[must_use]
    pub fn with_health(mut self, health: HealthOf) -> Self {
        self.health = Some(health);
        self
    }

    /// `/api/v1/health`.
    pub(super) fn health(&self, target: &str) -> Answer {
        match &self.health {
            Some(h) => Answer::json(200, json(&h())),
            None => Answer::error(target, 404, "not-found", "no health checks here"),
        }
    }
}
