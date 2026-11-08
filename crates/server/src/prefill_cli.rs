//! `lanstash prefill`, the pure parts: the admin API path that starts a
//! prefill, and one line of progress from `GET /api/v1/prefill`. The command
//! itself only talks to the running lanstash; the job runs there (and shows
//! in its web UI), never in this process.

use lanstash_domain::json::Value;

/// `POST` path that starts prefilling `platform/game`.
#[must_use]
pub fn start_path(platform: &str, game: &str, prerelease: bool, from_lancache: bool) -> String {
    let mut query = Vec::new();
    if prerelease {
        query.push("build=pre");
    }
    if from_lancache {
        query.push("from=lancache");
    }
    let q = if query.is_empty() { String::new() } else { format!("?{}", query.join("&")) };
    format!("/api/v1/prefill/{platform}/{game}{q}")
}

/// Where a prefill stands, read from the API's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub game: String,
    pub state: String,
    /// One line for the terminal.
    pub line: String,
}

impl Progress {
    #[must_use]
    pub fn running(&self) -> bool {
        self.state == "running"
    }
}

fn number(v: &Value, key: &str) -> u64 {
    match v.get(key) {
        Some(Value::Number(n)) if *n >= 0.0 => *n as u64,
        _ => 0,
    }
}

fn gb(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1e9)
}

/// The progress of the last prefill (`GET /api/v1/prefill`), `None` when
/// there is none or the answer is not one.
#[must_use]
pub fn progress(v: &Value) -> Option<Progress> {
    let game = v.get("game")?.as_str()?.to_owned();
    let state = v.get("state")?.as_str()?.to_owned();
    let from = v.get("from").and_then(Value::as_str).unwrap_or("cdn");
    // Of what the stash lacked: what it held already is not downloaded.
    let left = number(v, "bytes_total").saturating_sub(number(v, "bytes_skipped")).max(number(v, "bytes"));
    let mut line = format!(
        "{game} {}: {state}, {}/{} requests, {} of {}",
        v.get("version").and_then(Value::as_str).unwrap_or("?"),
        number(v, "files_done"),
        number(v, "files_total"),
        gb(number(v, "bytes")),
        gb(left),
    );
    if from == "lancache" {
        line.push_str(&format!(
            ", from lancache {} ({} requests, {} rejected)",
            gb(number(v, "bytes_local")),
            number(v, "files_local"),
            number(v, "files_rejected")
        ));
    }
    let skipped = number(v, "files_skipped");
    if skipped > 0 {
        line.push_str(&format!(", {skipped} already stashed"));
        // Their bytes, from a lanstash that says them.
        if v.get("bytes_skipped").is_some() {
            line.push_str(&format!(" ({})", gb(number(v, "bytes_skipped"))));
        }
    }
    let missing = number(v, "files_missing");
    if missing > 0 {
        line.push_str(&format!(", {missing} not on the CDN"));
    }
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        line.push_str(&format!(": {e}"));
    }
    Some(Progress { game, state, line })
}
