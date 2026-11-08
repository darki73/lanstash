//! Tests for `lanstash_adapter_admin_api::api::jobs`: prefill and game
//! deletion endpoints.

use super::*;

#[test]
fn prefill_paths_start_stop_and_report_across_platforms() {
    let old = Jobs::new(Ok(()), 10);
    let new = Jobs::new(Ok(()), 20);
    let busy = Jobs::new(Err(StartError::Busy), 0);
    let a = api().with_platforms(vec![
        Arc::new(Echo { id: "blizzard", views: false, jobs: Some(Arc::clone(&old)) }),
        Arc::new(Echo { id: "steam", views: false, jobs: Some(Arc::clone(&new)) }),
        Arc::new(Echo { id: "epic", views: false, jobs: Some(Arc::clone(&busy)) }),
        Arc::new(Echo { id: "riot", views: false, jobs: None }),
    ]);
    assert_eq!(get(&a, "GET", "/api/v1/prefill").body, "null\n", "none yet");
    let started = get(&a, "POST", "/api/v1/prefill/blizzard/osi?build=pre");
    assert_eq!((started.status, started.body.as_str()), (202, "{\"started\":\"osi\"}\n"));
    assert_eq!(*old.asked.lock().unwrap(), vec![("osi".to_owned(), true, PrefillFrom::Cdn)]);
    assert!(
        get(&a, "GET", "/api/v1/prefill")
            .body
            .starts_with("{\"platform\":\"blizzard\",\"game\":\"osi\",\"version\":null,\"prerelease\":true"),
        "the job says its platform, so a hidden game's job is masked"
    );
    get(&a, "POST", "/api/v1/prefill/steam/7?from=lancache");
    assert_eq!(new.asked.lock().unwrap()[0].2, PrefillFrom::Lancache);
    get(&a, "POST", "/api/v1/prefill/steam/8?from=lancache-only");
    assert_eq!(new.asked.lock().unwrap()[1].2, PrefillFrom::LancacheOnly);
    assert!(
        get(&a, "GET", "/api/v1/prefill").body.starts_with("{\"platform\":\"steam\",\"game\":\"8\""),
        "the one started last"
    );
    assert!(
        get(&a, "GET", "/api/v1/status").body.contains(",\"prefill\":{\"platform\":\"steam\",\"game\":\"8\""),
        "in the snapshot"
    );
    let b = get(&a, "POST", "/api/v1/prefill/epic/x");
    assert_eq!((b.status, b.body.as_str()), (409, "{\"error\":\"a prefill is running\",\"kind\":\"busy\"}\n"));
    for target in ["/api/v1/prefill/riot/x", "/api/v1/prefill/nope/x", "/api/v1/prefill/blizzard/a%2Fb"] {
        assert_eq!(get(&a, "POST", target).status, 404, "{target}");
    }
    // Another install: refused by a platform that prefills only its own, and only tag names pass.
    let other = get(&a, "POST", "/api/v1/prefill/steam/9?install=OSX%2Carm64");
    assert_eq!(other.status, 400, "{}", other.body);
    assert!(other.body.contains("only the install it is set up for"), "{}", other.body);
    assert_eq!(get(&a, "POST", "/api/v1/prefill/steam/9?install=a/b").status, 400);
    assert_eq!(get(&a, "POST", "/api/v1/prefill/steam/9?install=").status, 400);
    let snap = get(&a, "GET", "/api/v1/status").body;
    assert!(snap.contains(",\"prefills\":{\"blizzard\":{\"game\":\"osi\""), "each platform's own: {snap}");
    assert!(snap.contains("},\"steam\":{\"game\":\"8\""), "{snap}");
    let gone = get(&a, "DELETE", "/api/v1/prefill/steam/waiting");
    assert_eq!(
        (gone.status, gone.body.as_str()),
        (
            200,
            "{\"unqueued\":\"waiting\"}
"
        )
    );
    assert_eq!(get(&a, "DELETE", "/api/v1/prefill/steam/other").status, 404, "not queued");
    assert_eq!(get(&a, "DELETE", "/api/v1/prefill/riot/waiting").status, 404, "no prefill");
    let one = get(&a, "POST", "/api/v1/prefill/stop?platform=steam");
    assert_eq!(one.status, 202);
    assert!(*new.stopped.lock().unwrap() && !*old.stopped.lock().unwrap(), "only Steam's");
    assert_eq!(get(&a, "POST", "/api/v1/prefill/stop?platform=riot").status, 404);
    let stop = get(&a, "POST", "/api/v1/prefill/stop");
    assert_eq!((stop.status, stop.body.as_str()), (202, "{\"stopping\":true}\n"));
    assert!(*old.stopped.lock().unwrap() && *new.stopped.lock().unwrap() && *busy.stopped.lock().unwrap());
}

#[test]
fn prefill_start_errors_say_why() {
    let cases = [
        (StartError::NotAGame, 404, "the launcher's own data, not a game"),
        (StartError::NotSetUp, 404, "prefill is not set up"),
        (StartError::NoLancache, 404, "no lancache cache is set up (nginx_cache)"),
        (StartError::Spawn("no threads".into()), 500, "no threads"),
    ];
    for (e, status, message) in cases {
        let a = api().with_platforms(vec![Arc::new(Echo {
            id: "blizzard",
            views: false,
            jobs: Some(Jobs::new(Err(e), 0)),
        })]);
        let r = get(&a, "POST", "/api/v1/prefill/blizzard/osi");
        assert_eq!(r.status, status);
        assert!(r.body.contains(message), "{}", r.body);
    }
}

#[test]
fn a_game_can_be_deleted() {
    use lanstash_app::delete_game::GameDeleter;
    use std::time::Instant;
    assert_eq!(get(&api(), "DELETE", "/api/v1/games/blizzard/sc1live").status, 404, "not enabled");
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    let deleter = Arc::new(GameDeleter::new(Arc::new(store), Arc::new(NoEvents), Arc::new(FixedClock(1))));
    let jobs = Jobs::new(Ok(()), 1);
    let a = api()
        .with_platforms(vec![Arc::new(Echo { id: "blizzard", views: false, jobs: Some(Arc::clone(&jobs)) })])
        .with_deleter(deleter);
    let d = get(&a, "DELETE", "/api/v1/games/blizzard/sc1live");
    assert_eq!((d.status, d.body.as_str()), (202, "{\"deleting\":\"sc1live\",\"platform\":\"blizzard\"}\n"));
    assert_eq!(*jobs.unqueued.lock().unwrap(), ["sc1live"], "a queued prefill of it would fetch it back");
    // Its progress is in the status snapshot.
    let until = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        let s = get(&a, "GET", "/api/v1/status").body;
        if !s.contains("\"state\":\"running\"") || Instant::now() > until {
            break s;
        }
        std::thread::yield_now();
    };
    assert!(
        snapshot.contains(
            ",\"deleting\":{\"platform\":\"blizzard\",\"game\":\"sc1live\",\"state\":\"done\",\"items_total\":0,"
        ),
        "{snapshot}"
    );
    // Not while the platform prefills that game.
    get(&a, "POST", "/api/v1/prefill/blizzard/osi");
    let busy = get(&a, "DELETE", "/api/v1/games/blizzard/osi");
    assert_eq!((busy.status, busy.body.contains("being prefilled")), (409, true), "{}", busy.body);
    assert_eq!(get(&a, "DELETE", "/api/v1/games/blizzard").status, 404, "no game");
    assert_eq!(get(&a, "DELETE", "/api/v1/games/blizzard/a.b").status, 404, "not a game directory");
}

/// A platform whose game `app` is made of directories `d1` and `d2`.
#[derive(Debug, Default)]
struct Parted(Mutex<Vec<String>>);

impl Platform for Parted {
    fn id(&self) -> &str {
        "steam"
    }
    fn parts(&self, game: &str) -> Option<Vec<String>> {
        self.0.lock().unwrap().push(game.to_owned());
        (game == "app").then(|| vec!["d1".to_owned(), "d2".to_owned()])
    }
    fn forget(&self, game: &str) {
        self.0.lock().unwrap().push(format!("forget {game}"));
    }
}

#[test]
fn a_game_of_several_parts_is_deleted_as_one() {
    use lanstash_app::delete_game::GameDeleter;
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), Arc::new(FixedClock(1)))
            .unwrap();
    let deleter = Arc::new(GameDeleter::new(Arc::new(store), Arc::new(NoEvents), Arc::new(FixedClock(1))));
    let parted = Arc::new(Parted::default());
    let a = api().with_platforms(vec![parted.clone()]).with_deleter(Arc::clone(&deleter));
    let d = get(&a, "DELETE", "/api/v1/games/steam/app");
    assert_eq!((d.status, d.body.as_str()), (202, "{\"deleting\":\"app\",\"platform\":\"steam\"}\n"));
    assert_eq!(*parted.0.lock().unwrap(), ["app", "forget app"], "asked for its parts, then told to forget it");
    assert_eq!(deleter.status().map(|s| s.game), Some("app".to_owned()));
}
