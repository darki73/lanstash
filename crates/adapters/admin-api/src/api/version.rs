//! `GET /api/v1/version`: `{"version": "..."}`, with `"latest": "..."` when a newer release is
//! known (asked of the [`Newer`] given: the composition root's watch of the releases).

use std::sync::Arc;

use lanstash_domain::json::string;

use super::{AdminApi, Answer, VERSION};

/// The release newer than the running version, when one is known (asking never waits).
pub type Newer = Arc<dyn Fn() -> Option<String> + Send + Sync>;

impl AdminApi {
    /// Says a newer release on `GET /api/v1/version`, asked of `newer`.
    #[must_use]
    pub fn with_releases(mut self, newer: Newer) -> Self {
        self.newer = Some(newer);
        self
    }

    /// `GET /api/v1/version`.
    pub(super) fn version(&self) -> Answer {
        let latest = self.newer.as_ref().and_then(|n| n()).map(|v| format!(",\"latest\":{}", string(&v)));
        Answer::json(200, format!("{{\"version\":{}{}}}\n", string(VERSION), latest.unwrap_or_default()))
    }
}
