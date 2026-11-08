//! The operator's text reports.

use std::collections::BTreeMap;

use lanstash::report::{VERIFY_PROBLEMS_SHOWN, import, usage, verify};
use lanstash_app::import_nginx::ImportReport;
use lanstash_app::verify::{GameVerify, VerifyReport};
use lanstash_ports::names::GameNames;
use lanstash_store::index::Usage;
use lanstash_store::layout::GameDir;

struct Names;

impl GameNames for Names {
    fn name(&self, service: &str, game: &str) -> Option<String> {
        (service == "steam" && game == "7").then(|| "Seven".to_owned())
    }
}

#[test]
fn import_lists_every_counter_and_the_sample_errors() {
    let r = ImportReport {
        files_listed: 10,
        imported_bytes: 2_000_000_000,
        records: 3,
        sample_errors: vec!["ab/cd/x: vanished".into()],
        ..ImportReport::default()
    };
    let text = import(&r, 2.0);
    assert!(text.starts_with("import finished in 2.0s\n"), "{text}");
    assert!(text.contains("  imported                  2.00 GB (1000 MB/s)\n"), "{text}");
    assert!(text.contains("  as segment records        3\n"), "{text}");
    assert!(text.ends_with("    e.g. ab/cd/x: vanished\n"), "{text}");
}

#[test]
fn usage_totals_each_service_and_names_games() {
    let mut u = BTreeMap::new();
    u.insert(GameDir::new("steam", "7"), Usage { objects: 2, bytes: 3_000_000_000, last_used_ms: 0 });
    u.insert(GameDir::new("steam", "8"), Usage { objects: 1, bytes: 1_000_000_000, last_used_ms: 0 });
    let text = usage(&u, &Names);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    assert!(lines[0].starts_with("steam") && lines[0].contains("3 objects") && lines[0].ends_with("4.00 GB"));
    assert!(lines[1].starts_with("  Seven (7)"), "{text}");
    assert!(lines[2].starts_with("  8 "), "{text}");
    assert_eq!(usage(&BTreeMap::new(), &Names), "store is empty\n");
}

#[test]
fn verify_shows_games_and_caps_the_problem_list() {
    let mut r = VerifyReport::default();
    r.games.insert(
        GameDir::new("steam", "7"),
        GameVerify { records: 4, record_bytes: 1_000_000_000, ..GameVerify::default() },
    );
    assert!(verify(&r, &Names, 1.0).ends_with("no problems: every stored byte matches its checksum\n"));
    r.problems =
        (0..VERIFY_PROBLEMS_SHOWN + 2).map(|i| (format!("steam/7/seg/{i:05}.lcseg"), "bad".to_owned())).collect();
    let text = verify(&r, &Names, 1.0);
    assert!(text.contains("Seven (steam/7)"), "{text}");
    assert!(text.contains(&format!("{} problem(s):", VERIFY_PROBLEMS_SHOWN + 2)), "{text}");
    assert!(text.ends_with("  … 2 more\n"), "{text}");
}
