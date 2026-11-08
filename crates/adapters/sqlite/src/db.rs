//! The database: opening it, its schema and moving it forward, and the lock
//! around its one connection.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::Connection;

/// The schema, step by step: step `n` takes `user_version` from `n` to `n + 1`. A step is never
/// changed once shipped; a change is a new step.
const STEPS: &[&str] = &[
    // 1: the build history, counters by the minute, prefill runs.
    "CREATE TABLE history_events (
        id INTEGER PRIMARY KEY,
        at_ms INTEGER NOT NULL,
        platform TEXT NOT NULL,
        game TEXT NOT NULL,
        change TEXT NOT NULL,
        version TEXT NOT NULL,
        from_version TEXT
    );
    CREATE INDEX history_events_by_game ON history_events (platform, game, at_ms);
    CREATE TABLE history_seen (
        platform TEXT NOT NULL,
        game TEXT NOT NULL,
        version TEXT NOT NULL,
        current INTEGER NOT NULL,
        PRIMARY KEY (platform, game)
    ) WITHOUT ROWID;
    CREATE TABLE history_platforms (platform TEXT PRIMARY KEY) WITHOUT ROWID;
    CREATE TABLE stats_minutes (
        minute_ms INTEGER NOT NULL,
        name TEXT NOT NULL,
        delta INTEGER NOT NULL,
        PRIMARY KEY (minute_ms, name)
    ) WITHOUT ROWID;
    CREATE TABLE prefill_runs (
        platform TEXT NOT NULL,
        game TEXT NOT NULL,
        started_ms INTEGER NOT NULL,
        finished_ms INTEGER,
        version TEXT,
        prerelease INTEGER NOT NULL,
        source TEXT NOT NULL,
        state TEXT NOT NULL,
        files_total INTEGER NOT NULL,
        files_done INTEGER NOT NULL,
        files_skipped INTEGER NOT NULL,
        bytes_skipped INTEGER NOT NULL,
        files_missing INTEGER NOT NULL,
        files_local INTEGER NOT NULL,
        bytes_local INTEGER NOT NULL,
        files_rejected INTEGER NOT NULL,
        bytes_total INTEGER NOT NULL,
        bytes INTEGER NOT NULL,
        error TEXT,
        PRIMARY KEY (platform, game, started_ms)
    );
    CREATE INDEX prefill_runs_by_start ON prefill_runs (started_ms);",
    // 2: the names given to clients.
    "CREATE TABLE client_labels (
        address TEXT PRIMARY KEY,
        label TEXT NOT NULL
    ) WITHOUT ROWID;",
];

/// The database.
pub struct Database {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// Opens (or makes) the database at `path` and brings its schema up to date.
    ///
    /// # Errors
    /// It could not be opened, or its schema is newer than this lanstash knows, or a step failed.
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        // Write-ahead log: a reader never waits on the writer; NORMAL is safe with it (a crash
        // loses at most the last commits, never the database).
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;")
            .map_err(|e| e.to_string())?;
        Self::ready(conn)
    }

    /// A database in memory (tests; nothing is kept).
    ///
    /// # Errors
    /// Its schema could not be made.
    pub fn in_memory() -> Result<Self, String> {
        Self::ready(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn ready(mut conn: Connection) -> Result<Self, String> {
        let at: usize = conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .try_into()
            .unwrap_or(0);
        if at > STEPS.len() {
            return Err(format!("the database is at schema {at}; this lanstash knows up to {}", STEPS.len()));
        }
        for (n, step) in STEPS.iter().enumerate().skip(at) {
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            tx.execute_batch(step).map_err(|e| format!("schema step {}: {e}", n + 1))?;
            tx.pragma_update(None, "user_version", int(n as u64 + 1)).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// The schema version the database is at.
    #[must_use]
    pub fn schema(&self) -> usize {
        self.lock()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .map_or(0, |v| v.try_into().unwrap_or(0))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A count or a time as SQLite keeps it (signed; what does not fit stays at the largest).
pub(crate) fn int(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// A count or a time read back (a negative one, which lanstash never writes, reads as 0).
pub(crate) fn uint(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}
