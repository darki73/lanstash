//! Tests for `lanstash_adapter_admin_api::api`'s history path: what was updated when.

use lanstash_app::build_history::BuildHistory;
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::platform::BuildStatus;

use super::*;

fn status(game: &str, version: &str, current: bool) -> BuildStatus {
    BuildStatus { game: game.into(), version: version.into(), known: true, current }
}

#[test]
fn the_history_newest_first_and_as_many_as_asked() {
    let history = Arc::new(BuildHistory::new(Arc::new(MemoryLogs::default())));
    history.record("steam", &[status("7", "1", true)], 10);
    history.record("steam", &[status("7", "2", false)], 20);
    history.record("steam", &[status("7", "2", true)], 30);
    let a = api().with_history(history);
    let got = get(&a, "GET", "/api/v1/history");
    assert_eq!(got.status, 200);
    assert_eq!(
        got.body,
        "{\"events\":[\
         {\"at_ms\":30,\"platform\":\"steam\",\"game\":\"7\",\"change\":\"current\",\"version\":\"2\",\"from\":null},\
         {\"at_ms\":20,\"platform\":\"steam\",\"game\":\"7\",\"change\":\"new-build\",\"version\":\"2\",\"from\":\"1\"}]}\n"
    );
    let one = get(&a, "GET", "/api/v1/history?limit=1");
    assert_eq!(one.body.matches("at_ms").count(), 1);
    assert!(one.body.contains("\"at_ms\":30"));
}

#[test]
fn without_a_history_the_path_is_not_found() {
    let got = get(&api(), "GET", "/api/v1/history");
    assert_eq!(got.status, 404);
    assert_eq!(get(&api(), "POST", "/api/v1/history").status, 405);
}
