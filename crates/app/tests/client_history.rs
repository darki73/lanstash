//! Tests for `lanstash_app::client_history`: each client's bytes per game over a
//! range, from the counters kept by the minute.

use std::sync::Arc;

use lanstash_app::client_history::{GameBytes, group, over};
use lanstash_app::counters_kept::{CounterSource, CountersKept, KeptOptions};
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::stats::StatsLog;

fn n(name: &str, v: u64) -> (String, u64) {
    (name.to_string(), v)
}

#[test]
fn counters_group_per_client_and_game_most_first() {
    let grouped = group(&[
        n("client/10.0.0.5/steam/7/hit", 300),
        n("client/10.0.0.5/steam/7/miss", 100),
        n("client/10.0.0.5/steam/8/hit", 50),
        n("client/10.0.0.6/blizzard/tpr/hs/miss", 900),
        n("served/steam/7", 5),
        n("client/broken", 7),
    ]);
    assert_eq!(grouped.len(), 2);
    assert_eq!((grouped[0].client.as_str(), grouped[0].hit, grouped[0].miss), ("10.0.0.6", 0, 900));
    let five = &grouped[1];
    assert_eq!((five.hit, five.miss), (350, 100));
    assert_eq!(
        five.games,
        [
            GameBytes { service: "steam".into(), game: "7".into(), hit: 300, miss: 100 },
            GameBytes { service: "steam".into(), game: "8".into(), hit: 50, miss: 0 },
        ]
    );
}

#[test]
fn a_range_is_read_from_the_kept_counters() {
    let logs = Arc::new(MemoryLogs::default());
    logs.add(0, &[n("client/10.0.0.5/steam/7/hit", 10), n("requests.hits", 3)]).unwrap();
    logs.add(60_000, &[n("client/10.0.0.5/steam/7/hit", 5)]).unwrap();
    logs.add(600_000, &[n("client/10.0.0.5/steam/7/miss", 99)]).unwrap();
    let source: CounterSource = Arc::new(Vec::new);
    let kept = CountersKept::new(source, logs, Arc::new(FixedClock(1)), KeptOptions::default());
    let got = over(&kept, 0, 120_000).unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].hit, got[0].miss), (15, 0), "only the range's minutes");
}
