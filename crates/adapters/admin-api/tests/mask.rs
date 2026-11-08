//! Tests for `lanstash_adapter_admin_api::mask`: hidden games masked by the shape of an answer.

use lanstash_adapter_admin_api::mask::Mask;

/// Steam app 10 is hidden (its depots 11 and 12 in the stash), and Battle.net's `wow`.
fn mask() -> Mask {
    let dirs = [
        ("steam".to_string(), "11".to_string(), Some("10".to_string())),
        ("steam".to_string(), "12".to_string(), Some("10".to_string())),
        ("steam".to_string(), "21".to_string(), Some("20".to_string())),
    ];
    Mask::new(&["blizzard/wow".into(), "steam/10".into()], &dirs)
}

#[test]
fn a_stash_entry_keeps_its_size_and_loses_its_name_and_ids() {
    let m = mask();
    let snapshot = r#"{"usage":[{"service":"steam","game":"11","name":"Secret","group":"10","objects":3,"bytes":500,"last_used_ms":9},{"service":"steam","game":"21","name":"Open","group":"20","objects":1,"bytes":7}]}"#;
    assert_eq!(
        m.json(snapshot, None, false),
        r#"{"usage":[{"service":"steam","game":"hidden-2","name":"A hidden game","group":"hidden-2","objects":3,"bytes":500,"last_used_ms":9,"hidden":true},{"service":"steam","game":"21","name":"Open","group":"20","objects":1,"bytes":7}]}"#
    );
}

#[test]
fn the_last_prefill_and_a_clients_counters_are_masked_by_their_platform() {
    let m = mask();
    let prefill = r#"{"prefill":{"platform":"steam","game":"10","version":"5","current":"depot/11/x","error":null,"state":"running","bytes":5,"queued":["20"]}}"#;
    assert_eq!(
        m.json(prefill, None, false),
        r#"{"prefill":{"platform":"steam","game":"hidden-2","version":"","current":"","error":null,"state":"running","bytes":5,"queued":["20"],"name":"A hidden game","hidden":true}}"#
    );
    let open = r#"{"platform":"steam","game":"20","queued":["10","20"]}"#;
    assert_eq!(m.json(open, None, false), r#"{"platform":"steam","game":"20","queued":["hidden-2","20"]}"#);
    let top = r#"{"top":[{"name":"client/192.0.2.7/steam/11/hit","value":9},{"name":"client/192.0.2.7/steam/21/miss","value":1}]}"#;
    assert_eq!(
        m.json(top, None, false),
        r#"{"top":[{"name":"client/192.0.2.7/steam/hidden-2/hit","value":9},{"name":"client/192.0.2.7/steam/21/miss","value":1}]}"#
    );
}

#[test]
fn history_loses_the_builds_and_keeps_what_changed() {
    let m = mask();
    let history = r#"{"events":[{"at_ms":1,"platform":"steam","game":"10","change":"new-build","version":"26510590","from":"24061265"}]}"#;
    assert_eq!(
        m.json(history, None, false),
        r#"{"events":[{"at_ms":1,"platform":"steam","game":"hidden-2","change":"new-build","version":"","from":"","name":"A hidden game","hidden":true}]}"#
    );
}

#[test]
fn a_platforms_lists_leave_hidden_games_out_and_owners_go_everywhere() {
    let m = mask();
    let catalog = r#"{"products":[{"code":"10","name":"Secret","game":"10","owners":["someone"]},{"code":"20","name":"Open","game":"20","owners":["someone"]}]}"#;
    assert_eq!(m.json(catalog, Some("steam"), true), r#"{"products":[{"code":"20","name":"Open","game":"20"}]}"#);
}

#[test]
fn counters_keys_and_lists_name_the_token() {
    let m = mask();
    let top = r#"{"top":[{"name":"served/steam/12","value":5},{"name":"served/steam/21","value":3}]}"#;
    assert_eq!(
        m.json(top, None, false),
        r#"{"top":[{"name":"served/steam/hidden-2","value":5},{"name":"served/steam/21","value":3}]}"#
    );
    let followed = r#"{"followed":["blizzard/wow","steam/20"]}"#;
    assert_eq!(m.json(followed, None, false), r#"{"followed":["blizzard/hidden-1","steam/20"]}"#);
    let downloads = r#"{"downloads":[{"key":"steam/11/chunk/ab","received":1}]}"#;
    assert_eq!(m.json(downloads, None, false), r#"{"downloads":[{"key":"steam/hidden-2","received":1}]}"#);
    let prefills = r#"{"prefills":{"steam":{"game":"10","current":"depot/11/x","state":"running","bytes":5,"planning":{"done":1},"queued":["10","20"]}}}"#;
    assert_eq!(
        m.json(prefills, None, false),
        r#"{"prefills":{"steam":{"game":"hidden-2","current":"","state":"running","bytes":5,"planning":null,"queued":["hidden-2","20"],"name":"A hidden game","hidden":true}}}"#
    );
}

#[test]
fn nothing_hidden_or_not_json_is_left_as_it_was() {
    let m = Mask::new(&[], &[]);
    assert!(m.is_empty());
    assert_eq!(m.json(r#"{"a":"steam/10"}"#, None, false), r#"{"a":"steam/10"}"#);
    assert_eq!(mask().json("not json", None, false), "not json");
    assert_eq!(mask().token("steam", "12"), Some("hidden-2"));
    assert_eq!(mask().token("steam", "21"), None);
}
