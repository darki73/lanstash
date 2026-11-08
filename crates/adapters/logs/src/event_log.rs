//! The event log: one line per [`Event`], to a file
//! or standard error. Writes are unbuffered so `tail -f` shows events at once.

use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use lanstash_ports::clock::Clock;

use lanstash_ports::events::{Event, Events};

use crate::rotating_file::{RotatingFile, Rotation};

/// One line (without newline) for `event` at `now_ms`.
#[must_use]
pub fn format_event(now_ms: u64, event: &Event<'_>) -> String {
    let span = |s: &Option<lanstash_domain::extent::Span>| {
        s.map_or_else(|| "-".to_owned(), |s| format!("{}-{}", s.start, s.end))
    };
    match event {
        Event::DownloadFailed { key, host, span: s, error } => {
            format!("{now_ms} download-failed host={host} key={key} span={} error={error:?}", span(s))
        }
        Event::ChecksumMismatch { key, host } => format!("{now_ms} checksum-mismatch host={host} key={key}"),
        Event::StoreFailed { key, error } => format!("{now_ms} store-failed key={key} error={error:?}"),
        Event::RangeIgnored { key, host } => format!("{now_ms} range-ignored host={host} key={key}"),
        Event::KeyMismatch { key } => format!("{now_ms} key-mismatch key={key}"),
        Event::TlsFailed { host, error } => format!("{now_ms} tls-failed host={host} error={error:?}"),
        Event::Evicted { path, bytes, last_used_ms } => {
            format!("{now_ms} evicted path={path} bytes={bytes} last-used={last_used_ms}")
        }
        Event::EvictFailed { path, error } => format!("{now_ms} evict-failed path={path} error={error:?}"),
        Event::Compacted { path, records, kept_bytes, freed_bytes } => {
            format!("{now_ms} compacted path={path} records={records} kept={kept_bytes} freed={freed_bytes}")
        }
    }
}

pub struct EventLog {
    out: Mutex<Box<dyn Write + Send>>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for EventLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLog").finish_non_exhaustive()
    }
}

impl EventLog {
    /// Appends to `path`, rotated by size.
    ///
    /// # Errors
    /// The file can't be opened.
    pub fn open(path: &Path, rotation: Rotation, clock: Arc<dyn Clock>) -> io::Result<Self> {
        let f = RotatingFile::open(path, rotation)?;
        Ok(Self { out: Mutex::new(Box::new(f)), clock })
    }

    #[must_use]
    pub fn stderr(clock: Arc<dyn Clock>) -> Self {
        Self { out: Mutex::new(Box::new(io::stderr())), clock }
    }
}

impl Events for EventLog {
    fn emit(&self, event: &Event<'_>) {
        let line = format_event(self.clock.now_ms(), event);
        let mut out = self.out.lock().unwrap_or_else(PoisonError::into_inner);
        // Losing an event line must never fail a request.
        // One write per line: a rotation never splits a line.
        let _ = out.write_all(
            format!(
                "{line}
"
            )
            .as_bytes(),
        );
    }
}
