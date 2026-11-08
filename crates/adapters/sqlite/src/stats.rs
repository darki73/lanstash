//! Counters over time in the database: a row per minute and counter that went up.

use lanstash_ports::stats::{Bucket, StatsLog};
use rusqlite::{params, params_from_iter, types::Value};

use crate::db::{Database, int, uint};

impl StatsLog for Database {
    fn add(&self, minute_ms: u64, deltas: &[(String, u64)]) -> Result<(), String> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (name, delta) in deltas.iter().filter(|(_, d)| *d > 0) {
            tx.execute(
                "INSERT INTO stats_minutes (minute_ms, name, delta) VALUES (?1, ?2, ?3)
                 ON CONFLICT (minute_ms, name) DO UPDATE SET delta = delta + excluded.delta",
                params![int(minute_ms), name, int(*delta)],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    fn series(&self, names: &[String], from_ms: u64, to_ms: u64, step_ms: u64) -> Result<Vec<Bucket>, String> {
        if names.is_empty() || to_ms <= from_ms {
            return Ok(Vec::new());
        }
        let step = step_ms.max(60_000);
        let sql = format!(
            "SELECT (minute_ms - ?1) / ?2 AS b, name, SUM(delta) FROM stats_minutes
             WHERE minute_ms >= ?1 AND minute_ms < ?3 AND name IN ({})
             GROUP BY b, name ORDER BY b",
            vec!["?"; names.len()].join(", ")
        );
        let mut args = vec![Value::Integer(int(from_ms)), Value::Integer(int(step)), Value::Integer(int(to_ms))];
        args.extend(names.iter().map(|n| Value::Text(n.clone())));
        let conn = self.lock();
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params_from_iter(args), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
            })
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| e.to_string())?;
        let mut out: Vec<Bucket> = Vec::new();
        for (b, name, sum) in rows {
            let start = from_ms + uint(b) * step;
            if out.last().is_none_or(|last| last.0 != start) {
                out.push((start, vec![0; names.len()]));
            }
            if let (Some(i), Some(last)) = (names.iter().position(|n| *n == name), out.last_mut()) {
                last.1[i] = uint(sum);
            }
        }
        Ok(out)
    }

    fn top(&self, prefix: &str, from_ms: u64, to_ms: u64, limit: usize) -> Result<Vec<(String, u64)>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT name, SUM(delta) AS s FROM stats_minutes
                 WHERE minute_ms >= ?1 AND minute_ms < ?2 AND substr(name, 1, ?3) = ?4
                 GROUP BY name ORDER BY s DESC, name LIMIT ?5",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(
                params![int(from_ms), int(to_ms), int(prefix.chars().count() as u64), prefix, int(limit as u64)],
                |r| Ok((r.get::<_, String>(0)?, uint(r.get::<_, i64>(1)?))),
            )
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    fn forget_before(&self, before_ms: u64) -> Result<u64, String> {
        self.lock()
            .execute("DELETE FROM stats_minutes WHERE minute_ms < ?1", [int(before_ms)])
            .map(|n| n as u64)
            .map_err(|e| e.to_string())
    }
}
