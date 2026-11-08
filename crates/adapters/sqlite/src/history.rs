//! The build history in the database: its events, the games as last seen, and
//! the platforms whose status was taken in.

use lanstash_ports::history::{Change, Event, EventQuery, HistoryLog, Seen, SeenAll};
use rusqlite::{params, params_from_iter, types::Value};

use crate::db::{Database, int, uint};

/// The events kept: the newest ones; older ones go as new ones come.
pub const EVENTS_KEPT: u64 = 50_000;

impl HistoryLog for Database {
    fn seen(&self) -> Result<SeenAll, String> {
        let conn = self.lock();
        let mut games =
            conn.prepare("SELECT platform, game, version, current FROM history_seen").map_err(|e| e.to_string())?;
        let games = games
            .query_map([], |r| {
                Ok(Seen { platform: r.get(0)?, game: r.get(1)?, version: r.get(2)?, held: held(r.get(3)?) })
            })
            .and_then(Iterator::collect)
            .map_err(|e| e.to_string())?;
        let mut platforms = conn.prepare("SELECT platform FROM history_platforms").map_err(|e| e.to_string())?;
        let platforms = platforms.query_map([], |r| r.get(0)).and_then(Iterator::collect).map_err(|e| e.to_string())?;
        Ok(SeenAll { games, platforms })
    }

    fn note(&self, platform: &str, events: &[Event], seen: &[Seen]) -> Result<(), String> {
        let mut conn = self.lock();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT OR IGNORE INTO history_platforms (platform) VALUES (?1)", [platform])
            .map_err(|e| e.to_string())?;
        for e in events {
            tx.execute(
                "INSERT INTO history_events (at_ms, platform, game, change, version, from_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![int(e.at_ms), e.platform, e.game, e.change.as_str(), e.version, e.from],
            )
            .map_err(|e| e.to_string())?;
        }
        if !events.is_empty() {
            tx.execute(
                "DELETE FROM history_events WHERE rowid NOT IN
                 (SELECT rowid FROM history_events ORDER BY at_ms DESC, rowid DESC LIMIT ?1)",
                [int(EVENTS_KEPT)],
            )
            .map_err(|e| e.to_string())?;
        }
        for s in seen {
            tx.execute(
                "INSERT INTO history_seen (platform, game, version, current) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (platform, game) DO UPDATE SET version = excluded.version, current = excluded.current",
                params![s.platform, s.game, s.version, held_code(s.held)],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    fn events(&self, query: &EventQuery<'_>) -> Result<Vec<Event>, String> {
        let mut sql =
            String::from("SELECT at_ms, platform, game, change, version, from_version FROM history_events WHERE 1 = 1");
        let mut args: Vec<Value> = Vec::new();
        if let Some(p) = query.platform {
            sql.push_str(" AND platform = ?");
            args.push(Value::Text(p.to_owned()));
        }
        if !query.games.is_empty() {
            sql.push_str(" AND game IN (");
            sql.push_str(&vec!["?"; query.games.len()].join(", "));
            sql.push(')');
            args.extend(query.games.iter().map(|g| Value::Text(g.clone())));
        }
        sql.push_str(" ORDER BY at_ms DESC, id DESC LIMIT ?");
        args.push(Value::Integer(int(query.limit as u64)));
        let conn = self.lock();
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params_from_iter(args), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| e.to_string())?;
        // A change this lanstash does not know (written by a newer one) is left out.
        Ok(rows
            .into_iter()
            .filter_map(|(at, platform, game, change, version, from)| {
                Some(Event { at_ms: uint(at), platform, game, change: Change::parse(&change)?, version, from })
            })
            .collect())
    }
}

/// `history_seen.current` as kept: 1 held whole, 0 not, 2 not known when taken in.
fn held_code(held: Option<bool>) -> i64 {
    held.map_or(2, i64::from)
}

fn held(code: i64) -> Option<bool> {
    match code {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
