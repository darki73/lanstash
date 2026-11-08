//! The names given to clients, by address.

use lanstash_ports::clients::ClientLabels;
use rusqlite::params;

use crate::db::Database;

impl ClientLabels for Database {
    fn labels(&self) -> Result<Vec<(String, String)>, String> {
        let conn = self.lock();
        let mut stmt =
            conn.prepare("SELECT address, label FROM client_labels ORDER BY address").map_err(|e| e.to_string())?;
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|e| e.to_string())
    }

    fn set_label(&self, address: &str, label: Option<&str>) -> Result<(), String> {
        let conn = self.lock();
        match label {
            Some(l) => conn.execute(
                "INSERT INTO client_labels (address, label) VALUES (?1, ?2)
                 ON CONFLICT (address) DO UPDATE SET label = excluded.label",
                params![address, l],
            ),
            None => conn.execute("DELETE FROM client_labels WHERE address = ?1", params![address]),
        }
        .map(drop)
        .map_err(|e| e.to_string())
    }
}
