//! Tests for `lanstash_adapter_sqlite::db`: the schema is made once, kept on
//! disk, and a newer one is refused.

use lanstash_adapter_sqlite::db::Database;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lanstash-sqlite-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_new_database_is_at_the_latest_schema() {
    assert_eq!(Database::in_memory().unwrap().schema(), 2);
}

#[test]
fn opened_again_it_keeps_its_schema() {
    let dir = scratch("again");
    let path = dir.join("lanstash.db");
    assert_eq!(Database::open(&path).unwrap().schema(), 2);
    assert_eq!(Database::open(&path).unwrap().schema(), 2, "no step runs twice");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_database_of_the_first_schema_is_moved_forward_keeping_what_it_holds() {
    use lanstash_ports::clients::ClientLabels;
    use lanstash_ports::stats::StatsLog;
    let dir = scratch("forward");
    let path = dir.join("lanstash.db");
    drop(Database::open(&path).unwrap());
    // As the first lanstash with a database left it: schema 1, without the clients' names.
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("DROP TABLE client_labels; PRAGMA user_version = 1;").unwrap();
        conn.execute("INSERT INTO stats_minutes (minute_ms, name, delta) VALUES (0, 'requests.hits', 4)", []).unwrap();
    }
    let db = Database::open(&path).unwrap();
    assert_eq!(db.schema(), 2);
    assert_eq!(db.top("requests.", 0, 60_000, 5).unwrap(), [("requests.hits".to_string(), 4)]);
    db.set_label("10.0.0.5", Some("Office")).unwrap();
    assert_eq!(db.labels().unwrap().len(), 1);
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_database_from_a_newer_lanstash_is_refused() {
    let dir = scratch("newer");
    let path = dir.join("lanstash.db");
    drop(Database::open(&path).unwrap());
    // What a newer lanstash would leave: a step this one does not know. SQLite keeps
    // `user_version` in the file's header, bytes 60..64, big-endian (closing checkpointed the log).
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[60..64].copy_from_slice(&99u32.to_be_bytes());
    std::fs::write(&path, bytes).unwrap();
    let refused = Database::open(&path).unwrap_err();
    assert!(refused.contains("schema 99"), "{refused}");
    let _ = std::fs::remove_dir_all(&dir);
}
