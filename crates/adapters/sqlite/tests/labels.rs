//! Tests for `lanstash_adapter_sqlite::labels`: the names given to clients.

use lanstash_adapter_sqlite::db::Database;
use lanstash_ports::clients::ClientLabels;

#[test]
fn a_name_is_given_changed_and_taken_away() {
    let db = Database::in_memory().unwrap();
    assert!(db.labels().unwrap().is_empty());
    db.set_label("10.0.0.6", Some("Office")).unwrap();
    db.set_label("10.0.0.5", Some("Living room")).unwrap();
    db.set_label("10.0.0.6", Some("Study")).unwrap();
    assert_eq!(
        db.labels().unwrap(),
        [("10.0.0.5".to_string(), "Living room".to_string()), ("10.0.0.6".to_string(), "Study".to_string())]
    );
    db.set_label("10.0.0.5", None).unwrap();
    db.set_label("10.0.0.9", None).unwrap();
    assert_eq!(db.labels().unwrap(), [("10.0.0.6".to_string(), "Study".to_string())]);
}
