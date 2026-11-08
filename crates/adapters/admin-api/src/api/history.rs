//! The history endpoint: `GET /api/v1/history[?limit=N]`, what was updated
//! when ([`BuildHistory`]), newest first: `{"events":[{"at_ms","platform",
//! "game","change","version","from"}]}`. 404 without a history.

use std::sync::Arc;

use lanstash_app::build_history::BuildHistory;
use lanstash_domain::json::string;
use lanstash_ports::history::Event;

use super::{AdminApi, Answer};

/// How many events an answer has unless the request asks for fewer.
const LIMIT: usize = 200;

impl AdminApi {
    /// Enables the history path.
    #[must_use]
    pub fn with_history(mut self, history: Arc<BuildHistory>) -> Self {
        self.history = Some(history);
        self
    }

    /// `/api/v1/history`.
    pub(super) fn history(&self, target: &str) -> Answer {
        let Some(history) = &self.history else {
            return Answer::error(target, 404, "not-found", "no build history");
        };
        let limit = target
            .split_once('?')
            .and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix("limit=")))
            .and_then(|n| n.parse::<usize>().ok())
            .map_or(LIMIT, |n| n.min(LIMIT));
        let events: Vec<String> = history.events(None, &[], limit).iter().map(event_json).collect();
        Answer::json(200, format!("{{\"events\":[{}]}}\n", events.join(",")))
    }
}

fn event_json(e: &Event) -> String {
    format!(
        "{{\"at_ms\":{},\"platform\":{},\"game\":{},\"change\":{},\"version\":{},\"from\":{}}}",
        e.at_ms,
        string(&e.platform),
        string(&e.game),
        string(e.change.as_str()),
        string(&e.version),
        e.from.as_deref().map_or_else(|| "null".to_owned(), string)
    )
}
