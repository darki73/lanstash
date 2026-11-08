//! Access log file: one line per request, appended and written immediately
//! (so `tail -f` shows requests live).
//!
//! ```text
//! 1793833200123 10.0.1.100 GET level3.blizzard.com /tpr/wow/data/f4/43/f443… range=bytes=0-1048575 status=206 cache=MISS bytes=1048576 ua="Battle.net/1.0"
//! ```
//! (Example values; Battle.net actually sends no User-Agent.)
//! Fields are space-separated; `-` means absent. `INCOMPLETE` is appended when
//! the response was cut off. With [`FileAccessLog::with_all_headers`], every
//! request header follows as `hdr="name: value | name: value"`.

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Clock;

use crate::rotating_file::{RotatingFile, Rotation};
use lanstash_adapter_http::server::{AccessEntry, AccessLog};

/// Appends access lines to a file.
pub struct FileAccessLog {
    file: Mutex<RotatingFile>,
    clock: Arc<dyn Clock>,
    all_headers: bool,
}

impl std::fmt::Debug for FileAccessLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FileAccessLog")
    }
}

impl FileAccessLog {
    /// Opens (creating if needed) `path` for appending, rotated by size.
    ///
    /// # Errors
    /// The file can't be opened.
    pub fn open(path: &Path, rotation: Rotation, clock: Arc<dyn Clock>) -> std::io::Result<Self> {
        let file = RotatingFile::open(path, rotation)?;
        Ok(Self { file: Mutex::new(file), clock, all_headers: false })
    }

    /// Also log every request header (to see what a client really sends).
    #[must_use]
    pub fn with_all_headers(mut self, on: bool) -> Self {
        self.all_headers = on;
        self
    }
}

/// Keeps a quoted value on one line and free of quotes.
fn quoted(v: &str) -> String {
    v.chars().map(|c| if c == '"' || c.is_control() { '\'' } else { c }).collect()
}

/// Keeps a field on one line and free of separators.
fn field(v: &str) -> String {
    let s: String = v.chars().map(|c| if c.is_control() || c == ' ' || c == '"' { '_' } else { c }).collect();
    if s.is_empty() { "-".into() } else { s }
}

/// Formats one entry (without the trailing newline).
#[must_use]
pub fn format_entry(now_ms: u64, e: &AccessEntry<'_>, all_headers: bool) -> String {
    let peer = e.peer.map_or_else(|| "-".to_owned(), |p| p.ip().to_string());
    let ua = quoted(e.req.header("user-agent").unwrap_or("-"));
    let mut line = format!(
        "{now_ms} {peer} {} {} {} range={} status={} cache={} bytes={}{} ua=\"{ua}\"",
        field(&e.req.method),
        field(e.req.host().unwrap_or("-")),
        field(&e.req.target),
        field(e.req.header("range").unwrap_or("-")),
        e.status,
        field(e.cache.unwrap_or("-")),
        e.bytes,
        if e.complete { "" } else { " INCOMPLETE" },
    );
    if all_headers {
        let all: Vec<String> = e.req.headers.iter().map(|(n, v)| format!("{n}: {v}")).collect();
        line.push_str(&format!(" hdr=\"{}\"", quoted(&all.join(" | "))));
    }
    line
}

impl AccessLog for FileAccessLog {
    fn record(&self, entry: &AccessEntry<'_>) {
        let mut line = format_entry(self.clock.now_ms(), entry, self.all_headers);
        line.push('\n');
        let mut f = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        // Logging must never break serving: write errors are ignored.
        let _ = f.write_all(line.as_bytes());
    }
}
