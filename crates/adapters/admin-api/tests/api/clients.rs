//! Tests for `lanstash_adapter_admin_api::api`'s clients paths: who downloads what
//! now (in every snapshot), what each client got over a range, and their names.

use lanstash_app::client_names::ClientNames;
use lanstash_app::counters_kept::{CounterSource, CountersKept, KeptOptions};
use lanstash_app::serve::clients::ClientTraffic;
use lanstash_app::test_support::MemoryLogs;
use lanstash_ports::clients::ClientLabels;
use lanstash_ports::stats::StatsLog;

use super::*;

/// An API whose cache counts clients in `traffic`, with names kept in `log`.
fn counting(traffic: &Arc<ClientTraffic>, log: &Arc<MemoryLogs>) -> AdminApi {
    let clock = Arc::new(FixedClock(5));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let cache = Arc::new(
        Cache::new(
            Arc::clone(&store),
            HostRules::default(),
            &Profiles::default(),
            Arc::new(NoEvents),
            ServeOptions::default(),
        )
        .with_clients(Arc::clone(traffic)),
    );
    let status = Arc::new(Status::new(cache, store, clock.clone(), Arc::new(NoNames), 30_000));
    let names = Arc::new(ClientNames::new(log.clone(), None, clock).unwrap());
    let source: CounterSource = Arc::new(Vec::new);
    let stats = Arc::new(CountersKept::new(source, log.clone(), Arc::new(FixedClock(1)), KeptOptions::default()));
    AdminApi::new(status, Duration::from_secs(1)).with_stats(stats).with_clients(names)
}

#[test]
fn the_clients_downloading_now_are_in_every_snapshot_with_their_names() {
    let traffic = Arc::new(ClientTraffic::new(Arc::new(FixedClock(5)), 10_000));
    let log = Arc::new(MemoryLogs::default());
    log.set_label("10.0.0.5", Some("Office")).unwrap();
    let a = counting(&traffic, &log);
    traffic.add("10.0.0.5", "steam", "7", true, 20_000);
    let got = get(&a, "GET", "/api/v1/status");
    assert!(
        got.body.contains(
            "\"clients\":[{\"address\":\"10.0.0.5\",\"label\":\"Office\",\"host\":null,\"service\":\"steam\",\"game\":\"7\",\"per_sec\":2000,\"hit_bytes\":20000,\"miss_bytes\":0,\"total_bytes\":20000,\"session_bytes\":20000,\"since_ms\":5,\"last_ms\":5}]"
        ),
        "{}",
        got.body
    );
    assert!(get(&api(), "GET", "/api/v1/status").body.contains("\"clients\":[]"), "none counted: none");
}

#[test]
fn what_each_client_got_over_a_range() {
    let traffic = Arc::new(ClientTraffic::new(Arc::new(FixedClock(5)), 10_000));
    let log = Arc::new(MemoryLogs::default());
    log.add(0, &[("client/10.0.0.5/steam/7/hit".into(), 300), ("client/10.0.0.5/steam/7/miss".into(), 100)]).unwrap();
    let a = counting(&traffic, &log);
    let got = get(&a, "GET", "/api/v1/clients?from=0&to=60000");
    assert_eq!(got.status, 200, "{}", got.body);
    assert_eq!(
        got.body,
        "{\"clients\":[{\"address\":\"10.0.0.5\",\"label\":null,\"host\":null,\"hit\":300,\"miss\":100,\"games\":[{\"service\":\"steam\",\"game\":\"7\",\"hit\":300,\"miss\":100}]}]}\n"
    );
    assert_eq!(get(&a, "GET", "/api/v1/clients?from=5&to=5").status, 400);
    assert_eq!(get(&a, "GET", "/api/v1/clients").status, 400);
    assert_eq!(get(&api(), "GET", "/api/v1/clients?from=0&to=1").status, 404, "not counted");
}

#[test]
fn a_client_is_named_renamed_and_its_name_taken_away() {
    let traffic = Arc::new(ClientTraffic::new(Arc::new(FixedClock(5)), 10_000));
    let log = Arc::new(MemoryLogs::default());
    let a = counting(&traffic, &log);
    let got = get(&a, "PUT", "/api/v1/clients/10.0.0.5?label=Living%20room");
    assert_eq!(got.status, 200, "{}", got.body);
    assert_eq!(got.body, "{\"address\":\"10.0.0.5\",\"label\":\"Living room\",\"host\":null}\n");
    assert_eq!(log.labels().unwrap(), [("10.0.0.5".to_string(), "Living room".to_string())]);
    assert_eq!(get(&a, "DELETE", "/api/v1/clients/10.0.0.5").status, 200);
    assert!(log.labels().unwrap().is_empty());
    assert_eq!(get(&a, "PUT", "/api/v1/clients/not-an-address?label=x").status, 400);
    let v6 = get(&a, "PUT", "/api/v1/clients/fd00%3A%3A5?label=Den");
    assert_eq!(v6.status, 200, "an escaped IPv6 address: {}", v6.body);
    assert_eq!(log.labels().unwrap(), [("fd00::5".to_string(), "Den".to_string())]);
    assert_eq!(get(&a, "GET", "/api/v1/clients/10.0.0.5").status, 405);
}
