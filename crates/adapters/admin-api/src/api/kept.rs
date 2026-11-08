//! What is kept beyond a restart, through the API:
//! - `GET /api/v1/stats?names=a,b&from=<ms>&to=<ms>[&step=<ms>]`: counters over time, as how
//!   much each went up per bucket: `{"step_ms","names":[..],"buckets":[{"at_ms","values":[..]}]}`;
//! - `GET /api/v1/prefill/runs[?platform=<id>][&limit=N]`: the prefill jobs that ended, newest
//!   first: `{"runs":[{"platform","job":{..}}]}`;
//! - after a restart, a platform's last prefill in the status is the last one kept, until a new one starts.

use std::sync::Arc;

use lanstash_app::counters_kept::{CountersKept, step};
use lanstash_app::prefill_runs::PrefillRuns;
use lanstash_domain::json::string;
use lanstash_ports::prefill::PrefillStatus;

use super::{AdminApi, Answer};
use crate::render::prefill_json;

/// The most runs an answer has.
const RUNS: usize = 200;

/// The value of `key` in `target`'s query.
fn param<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    let (_, q) = target.split_once('?')?;
    q.split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}

impl AdminApi {
    /// Enables `/api/v1/stats`.
    #[must_use]
    pub fn with_stats(mut self, stats: Arc<CountersKept>) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Enables `/api/v1/prefill/runs`, and the last kept job of a platform with none since the start.
    #[must_use]
    pub fn with_prefill_runs(mut self, runs: Arc<PrefillRuns>) -> Self {
        self.runs = Some(runs);
        self
    }

    /// `/api/v1/stats`.
    pub(super) fn stats(&self, target: &str) -> Answer {
        let Some(stats) = &self.stats else {
            return Answer::error(target, 404, "not-found", "no counters are kept");
        };
        let num = |k| param(target, k).and_then(|v| v.parse::<u64>().ok());
        let names: Vec<String> =
            param(target, "names").unwrap_or("").split(',').filter(|n| !n.is_empty()).map(String::from).collect();
        let (Some(from), Some(to)) = (num("from"), num("to")) else {
            return Answer::error(target, 400, "bad-request", "from and to (ms) are needed");
        };
        if names.is_empty() || names.len() > 32 || to <= from {
            return Answer::error(target, 400, "bad-request", "1 to 32 names, and from before to");
        }
        let asked = num("step").unwrap_or(60_000);
        match stats.series(&names, from, to, asked) {
            Ok(buckets) => {
                let step = step(from, to, asked);
                let rows: Vec<String> = buckets
                    .iter()
                    .map(|(at, values)| {
                        let v: Vec<String> = values.iter().map(u64::to_string).collect();
                        format!("{{\"at_ms\":{at},\"values\":[{}]}}", v.join(","))
                    })
                    .collect();
                let names: Vec<String> = names.iter().map(|n| string(n)).collect();
                Answer::json(
                    200,
                    format!(
                        "{{\"step_ms\":{step},\"names\":[{}],\"buckets\":[{}]}}\n",
                        names.join(","),
                        rows.join(",")
                    ),
                )
            }
            Err(e) => Answer::error(target, 503, "unavailable", &e),
        }
    }

    /// `/api/v1/stats/top?prefix=&from=&to=[&limit=]`: the counters named `prefix…` that went up
    /// most in the range, most first: `{"top":[{"name","value"}]}`.
    pub(super) fn stats_top(&self, target: &str) -> Answer {
        let Some(stats) = &self.stats else {
            return Answer::error(target, 404, "not-found", "no counters are kept");
        };
        let num = |k| param(target, k).and_then(|v| v.parse::<u64>().ok());
        let (Some(from), Some(to), Some(prefix)) = (num("from"), num("to"), param(target, "prefix")) else {
            return Answer::error(target, 400, "bad-request", "prefix, from and to (ms) are needed");
        };
        let prefix = prefix.replace("%2F", "/").replace("%2f", "/");
        let limit = num("limit").map_or(20, |n| n.min(100) as usize);
        match stats.top(&prefix, from, to, limit) {
            Ok(top) => {
                let rows: Vec<String> =
                    top.iter().map(|(n, v)| format!("{{\"name\":{},\"value\":{v}}}", string(n))).collect();
                Answer::json(200, format!("{{\"top\":[{}]}}\n", rows.join(",")))
            }
            Err(e) => Answer::error(target, 503, "unavailable", &e),
        }
    }

    /// `/api/v1/prefill/runs`.
    pub(super) fn prefill_runs(&self, target: &str) -> Answer {
        let Some(runs) = &self.runs else {
            return Answer::error(target, 404, "not-found", "no prefill jobs are kept");
        };
        let limit = param(target, "limit").and_then(|n| n.parse::<usize>().ok()).map_or(RUNS, |n| n.min(RUNS));
        let rows: Vec<String> = runs
            .runs(param(target, "platform"), limit)
            .iter()
            .map(|(p, job)| format!("{{\"platform\":{},\"job\":{}}}", string(p), prefill_json(job)))
            .collect();
        Answer::json(200, format!("{{\"runs\":[{}]}}\n", rows.join(",")))
    }

    /// Each platform's last prefill: the running or last one of this run, else the last one kept.
    pub(super) fn last_prefills(&self) -> Vec<(String, PrefillStatus)> {
        self.platforms
            .iter()
            .filter_map(|p| {
                let prefill = p.prefill()?;
                let last = prefill.status().or_else(|| self.runs.as_ref().and_then(|r| r.last(p.id())))?;
                Some((p.id().to_owned(), last))
            })
            .collect()
    }
}
