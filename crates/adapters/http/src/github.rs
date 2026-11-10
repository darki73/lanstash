//! lanstash's releases on GitHub, for the [`Releases`] port: the newest release's tag from
//! `GET https://api.github.com/repos/<repo>/releases/latest` (`tag_name`, `v0.2.0` → `0.2.0`), through
//! the [`Web`] port. A repository with no release yet answers 404: no release known.

use std::sync::Arc;

use lanstash_domain::json;
use lanstash_ports::releases::Releases;
use lanstash_ports::web::{Tls, Web, WebRequest};

/// GitHub's API.
pub const API_HOST: &str = "api.github.com";

/// The releases of one repository (`owner/name`).
pub struct GitHubReleases {
    web: Arc<dyn Web>,
    repo: String,
    /// Sent as `User-Agent`, which GitHub asks every client for (`lanstash/0.2.0`).
    agent: String,
}

impl GitHubReleases {
    #[must_use]
    pub fn new(web: Arc<dyn Web>, repo: &str, agent: &str) -> Self {
        Self { web, repo: repo.to_owned(), agent: agent.to_owned() }
    }
}

impl std::fmt::Debug for GitHubReleases {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHubReleases").field("repo", &self.repo).finish_non_exhaustive()
    }
}

impl Releases for GitHubReleases {
    fn latest(&self) -> Result<Option<String>, String> {
        let target = format!("/repos/{}/releases/latest", self.repo);
        let headers = [("User-Agent", self.agent.as_str()), ("Accept", "application/vnd.github+json")];
        let req = WebRequest {
            method: "GET",
            host: API_HOST,
            port: 443,
            target: &target,
            headers: &headers,
            body: &[],
            tls: Some(Tls { pin: None, unchecked: false }),
        };
        let res = self.web.send(&req).map_err(|e| format!("{API_HOST}: {e}"))?;
        match res.status {
            200 => {}
            404 => return Ok(None),
            s => return Err(format!("{API_HOST}: HTTP {s}")),
        }
        let text = String::from_utf8_lossy(&res.body);
        let doc = json::parse(&text).map_err(|e| format!("{API_HOST}: {e:?}"))?;
        let tag = doc.get("tag_name").and_then(json::Value::as_str).ok_or(format!("{API_HOST}: no tag_name"))?;
        Ok(Some(tag.strip_prefix('v').unwrap_or(tag).to_owned()))
    }
}
