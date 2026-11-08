//! The generic documents: snapshots, rates, prefill status, errors.

use std::collections::BTreeMap;

use lanstash_adapter_admin_api::render::{delete_json, error_json, prefill_json, rates, render_json};
use lanstash_app::client_names::ClientName;
use lanstash_app::delete_game::{DeleteState, DeleteStatus};
use lanstash_app::fetch::DownloadInfo;
use lanstash_app::serve::clients::Downloading;
use lanstash_app::status::{CounterGroup, Snapshot, UsageRow};
use lanstash_domain::extent::Span;
use lanstash_ports::prefill::{Planning, PrefillFrom, PrefillStatus};
use lanstash_store::index::Usage;
use lanstash_store::layout::GameDir;

fn snapshot() -> Snapshot {
    Snapshot {
        now_ms: 3_725_000,
        started_ms: 0,
        counters: vec![CounterGroup { name: "requests", values: vec![("hits", 3), ("hit_bytes", 2_500_000_000)] }],
        downloads: vec![
            DownloadInfo {
                key: "blizzard/tpr/<x>\"q".into(),
                total: Some(1000),
                span: Some(Span::new(0, 200)),
                received: 50,
            },
            DownloadInfo { key: "steam/x".into(), total: None, span: Some(Span::new(0, u64::MAX)), received: 9 },
        ],
        usage: vec![UsageRow {
            game: GameDir::new("steam", "7"),
            usage: Usage { objects: 2, bytes: 3_000_000_000, last_used_ms: 3_600_000 },
            name: Some("Seven \"7\"".into()),
            group: Some("7".into()),
        }],
        usage_ms: 3_715_000,
        prefill: None,
        prefills: Vec::new(),
        deleting: None,
        builds_changed: vec![("steam".into(), 7)],
        clients: vec![(
            Downloading {
                client: "fd00::5".into(),
                service: "steam".into(),
                game: "7".into(),
                per_sec: 1_000,
                hit_bytes: 6_000,
                miss_bytes: 4_000,
                total_bytes: 50_000,
                session_bytes: 30_000,
                since_ms: 3_700_000,
                last_ms: 3_724_000,
            },
            ClientName { label: Some("Den \"PC\"".into()), host: None },
        )],
    }
}

#[test]
fn json_is_well_formed() {
    let j = render_json(&snapshot(), &BTreeMap::from([("requests.hit_bytes".to_owned(), 12.5)]));
    assert!(j.contains(r#""requests":{"hits":3,"hit_bytes":2500000000}"#), "{j}");
    assert!(j.contains(r#""rates_mb_s":{"requests.hit_bytes":12.500}"#), "{j}");
    assert!(j.contains(r#""key":"blizzard/tpr/<x>\"q","span":[0,200],"received":50,"total":1000"#), "{j}");
    assert!(j.contains(r#""span":[0,null],"received":9,"total":null"#), "an unknown end is null: {j}");
    assert!(
        j.contains(r#"{"service":"steam","game":"7","name":"Seven \"7\"","group":"7","objects":2,"bytes":3000000000,"last_used_ms":3600000}"#),
        "{j}"
    );
    assert!(
        j.ends_with(
            ",\"prefill\":null,\"prefills\":{},\"deleting\":null,\"builds_changed\":{\"steam\":7},\"clients\":[{\"address\":\"fd00::5\",\"label\":\"Den \\\"PC\\\"\",\"host\":null,\"service\":\"steam\",\"game\":\"7\",\"per_sec\":1000,\"hit_bytes\":6000,\"miss_bytes\":4000,\"total_bytes\":50000,\"session_bytes\":30000,\"since_ms\":3700000,\"last_ms\":3724000}]}"
        ),
        "{j}"
    );
    let depth = j.chars().fold(0i32, |d, c| match c {
        '{' | '[' => d + 1,
        '}' | ']' => d - 1,
        _ => d,
    });
    assert_eq!(depth, 0);
}

#[test]
fn rates_of_byte_counters_only() {
    let g = |hits, bytes| vec![CounterGroup { name: "requests", values: vec![("hits", hits), ("hit_bytes", bytes)] }];
    let (r, sample) = rates(None, 1_000, &g(1, 0));
    assert!(r.is_empty(), "no rate without a previous sample");
    let (r, _) = rates(Some(&sample), 3_000, &g(5, 20_000_000));
    assert_eq!(r, BTreeMap::from([("requests.hit_bytes".to_owned(), 10.0)]));
}

#[test]
fn a_prefill_has_every_field_the_web_app_reads() {
    let p = PrefillStatus {
        version: Some("3.3.7".into()),
        files_total: 4,
        files_done: 2,
        files_skipped: 1,
        bytes_skipped: 25,
        files_local: 1,
        bytes_local: 30,
        files_rejected: 1,
        bytes_total: 100,
        bytes: 40,
        planning: Some(Planning { done: 3, total: 9 }),
        queued: vec!["hs".into(), "w\"3".into()],
        install: Some("OSX,arm64".into()),
        ..PrefillStatus::started("osi", false, PrefillFrom::Lancache, 7)
    };
    assert_eq!(
        prefill_json(&p),
        "{\"game\":\"osi\",\"version\":\"3.3.7\",\"prerelease\":false,\"install\":\"OSX,arm64\",\"from\":\"lancache\",\"state\":\"running\",\
         \"files_total\":4,\"files_done\":2,\"files_skipped\":1,\"bytes_skipped\":25,\"files_missing\":0,\"files_local\":1,\
         \"bytes_local\":30,\"files_rejected\":1,\"bytes_total\":100,\"bytes\":40,\"current\":null,\"error\":null,\
         \"started_ms\":7,\"finished_ms\":null,\"planning\":{\"done\":3,\"total\":9},\"queued\":[\"hs\",\"w\\\"3\"]}"
    );
}

#[test]
fn a_deletion_has_every_field_the_web_app_reads() {
    let d = DeleteStatus {
        platform: "blizzard".into(),
        game: "sc1live".into(),
        state: DeleteState::Done,
        items_total: 3,
        items_done: 3,
        bytes_total: 900,
        bytes_done: 800,
        failed: vec![("a".into(), "gone".into())],
        started_ms: 5,
        finished_ms: Some(9),
        partial: true,
        bytes_kept: 40,
    };
    assert_eq!(
        delete_json(&d),
        "{\"platform\":\"blizzard\",\"game\":\"sc1live\",\"state\":\"done\",\"items_total\":3,\"items_done\":3,\
         \"bytes_total\":900,\"bytes_done\":800,\"failed\":1,\"started_ms\":5,\"finished_ms\":9,\"partial\":true,\"bytes_kept\":40}"
    );
}

#[test]
fn errors_say_what_and_which_kind() {
    assert_eq!(error_json("not-found", "no \"x\""), "{\"error\":\"no \\\"x\\\"\",\"kind\":\"not-found\"}\n");
}
