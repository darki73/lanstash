//! Tests for `lanstash_adapter_admin_api::api`'s paths to what is kept beyond a
//! restart: counters over time, the prefill jobs that ended, and a platform's
//! last job after a restart.

use lanstash_app::counters_kept::{CounterSource, CountersKept, KeptOptions};
use lanstash_app::prefill_runs::PrefillRuns;
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::prefill::{PrefillLog, PrefillState};
use lanstash_ports::stats::StatsLog;

use super::*;

fn stats(log: &Arc<MemoryLogs>) -> Arc<CountersKept> {
    let source: CounterSource = Arc::new(Vec::new);
    Arc::new(CountersKept::new(source, log.clone(), Arc::new(FixedClock(1)), KeptOptions::default()))
}

#[test]
fn counters_over_time_in_buckets() {
    let log = Arc::new(MemoryLogs::default());
    log.add(0, &[("requests.hits".into(), 2)]).unwrap();
    log.add(60_000, &[("requests.hits".into(), 3), ("requests.misses".into(), 1)]).unwrap();
    let a = api().with_stats(stats(&log));
    let got = get(&a, "GET", "/api/v1/stats?names=requests.hits,requests.misses&from=0&to=120000&step=120000");
    assert_eq!(got.status, 200, "{}", got.body);
    assert_eq!(
        got.body,
        "{\"step_ms\":120000,\"names\":[\"requests.hits\",\"requests.misses\"],\"buckets\":[{\"at_ms\":0,\"values\":[5,1]}]}\n"
    );
    for bad in ["/api/v1/stats?names=a", "/api/v1/stats?from=0&to=1", "/api/v1/stats?names=a&from=5&to=5"] {
        assert_eq!(get(&a, "GET", bad).status, 400, "{bad}");
    }
    assert_eq!(get(&api(), "GET", "/api/v1/stats?names=a&from=0&to=1").status, 404, "none kept");
}

#[test]
fn the_counters_that_went_up_most() {
    let log = Arc::new(MemoryLogs::default());
    log.add(0, &[("served/steam/7".into(), 5), ("served/steam/8".into(), 9), ("requests.hits".into(), 99)]).unwrap();
    let a = api().with_stats(stats(&log));
    let got = get(&a, "GET", "/api/v1/stats/top?prefix=served%2F&from=0&to=60000&limit=5");
    assert_eq!(got.status, 200, "{}", got.body);
    assert_eq!(
        got.body,
        "{\"top\":[{\"name\":\"served/steam/8\",\"value\":9},{\"name\":\"served/steam/7\",\"value\":5}]}\n"
    );
    assert_eq!(get(&a, "GET", "/api/v1/stats/top?from=0&to=1").status, 400, "no prefix");
}

#[test]
fn the_prefill_jobs_that_ended_newest_first() {
    let log = Arc::new(MemoryLogs::default());
    for (p, g, at) in [("steam", "7", 1), ("blizzard", "osi", 2)] {
        let mut s = PrefillStatus::started(g, false, PrefillFrom::Cdn, at);
        s.state = PrefillState::Done;
        log.record(p, &s).unwrap();
    }
    let runs = Arc::new(PrefillRuns::new(Vec::new(), log));
    let a = api().with_prefill_runs(runs);
    let all = get(&a, "GET", "/api/v1/prefill/runs");
    assert_eq!(all.status, 200);
    assert!(all.body.starts_with("{\"runs\":[{\"platform\":\"blizzard\",\"job\":{\"game\":\"osi\""), "{}", all.body);
    let steam = get(&a, "GET", "/api/v1/prefill/runs?platform=steam&limit=5");
    assert_eq!(steam.body.matches("\"platform\"").count(), 1);
    assert_eq!(get(&api(), "GET", "/api/v1/prefill/runs").status, 404, "none kept");
}
