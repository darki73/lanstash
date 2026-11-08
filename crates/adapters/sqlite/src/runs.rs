//! The prefill jobs that ran, in the database: one row per platform, game and start.

use lanstash_ports::prefill::{PrefillFrom, PrefillLog, PrefillState, PrefillStatus};
use rusqlite::{Row, params};

use crate::db::{Database, int, uint};

const COLUMNS: &str = "platform, game, started_ms, finished_ms, version, prerelease, source, state, files_total, \
     files_done, files_skipped, bytes_skipped, files_missing, files_local, bytes_local, files_rejected, bytes_total, \
     bytes, error";

/// The jobs kept: the newest ones; older ones go as new ones come.
pub const RUNS_KEPT: u64 = 2_000;

impl PrefillLog for Database {
    fn record(&self, platform: &str, run: &PrefillStatus) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "DELETE FROM prefill_runs WHERE rowid NOT IN
             (SELECT rowid FROM prefill_runs ORDER BY started_ms DESC LIMIT ?1)",
            [int(RUNS_KEPT - 1)],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            &format!(
                "INSERT OR REPLACE INTO prefill_runs ({COLUMNS})
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"
            ),
            params![
                platform,
                run.game,
                int(run.started_ms),
                run.finished_ms.map(int),
                run.version,
                i64::from(run.prerelease),
                run.from.as_str(),
                run.state.as_str(),
                int(run.files_total),
                int(run.files_done),
                int(run.files_skipped),
                int(run.bytes_skipped),
                int(run.files_missing),
                int(run.files_local),
                int(run.bytes_local),
                int(run.files_rejected),
                int(run.bytes_total),
                int(run.bytes),
                run.error,
            ],
        )
        .map(drop)
        .map_err(|e| e.to_string())
    }

    fn runs(&self, platform: Option<&str>, limit: usize) -> Result<Vec<(String, PrefillStatus)>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM prefill_runs WHERE ?1 IS NULL OR platform = ?1
                 ORDER BY started_ms DESC LIMIT ?2"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![platform, int(limit as u64)], run)
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().flatten().collect())
    }
}

/// A kept run; `None` for one whose source or state this lanstash does not know.
fn run(r: &Row<'_>) -> rusqlite::Result<Option<(String, PrefillStatus)>> {
    let n = |i: usize| r.get::<_, i64>(i).map(uint);
    let (Some(from), Some(state)) =
        (PrefillFrom::parse(&r.get::<_, String>(6)?), PrefillState::parse(&r.get::<_, String>(7)?))
    else {
        return Ok(None);
    };
    let status = PrefillStatus {
        game: r.get(1)?,
        version: r.get(4)?,
        prerelease: r.get::<_, i64>(5)? != 0,
        // Which other install a run was for is not kept: a kept run reads as the usual one.
        install: None,
        from,
        state,
        files_total: n(8)?,
        files_done: n(9)?,
        files_skipped: n(10)?,
        bytes_skipped: n(11)?,
        files_missing: n(12)?,
        files_local: n(13)?,
        bytes_local: n(14)?,
        files_rejected: n(15)?,
        bytes_total: n(16)?,
        bytes: n(17)?,
        current: None,
        error: r.get(18)?,
        started_ms: n(2)?,
        finished_ms: r.get::<_, Option<i64>>(3)?.map(uint),
        planning: None,
        queued: Vec::new(),
    };
    Ok(Some((r.get(0)?, status)))
}
