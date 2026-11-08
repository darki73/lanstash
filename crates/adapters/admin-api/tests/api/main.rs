//! Tests for `lanstash_adapter_admin_api::api`: answers, the event stream,
//! art, and dispatch to the platforms.

mod access;
mod account;
mod answer;
mod clients;
mod dns;
mod events;
mod health;
mod hidden;
mod history;
mod invites;
mod jobs;
mod kept;
mod own;
mod private;
mod settings;
mod setup;
mod updates;
mod users;
mod web;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use lanstash_adapter_admin_api::api::{AdminApi, VERSION};
use lanstash_app::art::{Art, ArtOptions};
use lanstash_app::serve::{Cache, ConnState, RequestHandler, ResponseHead, ResponseSink, ServeOptions, SinkError};
use lanstash_app::status::Status;
use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream};
use lanstash_domain::http::RequestHead;
use lanstash_domain::service::Profiles;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::art::{ArtError, ArtKind, ArtSource};
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::names::NoNames;
use lanstash_ports::platform::{ClientOs, Platform, SetupFile, View, ViewError};
use lanstash_ports::prefill::{Prefill, PrefillFrom, PrefillStatus, StartError};
use lanstash_ports::upstream::Source;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

fn status() -> Arc<Status> {
    let clock = Arc::new(FixedClock(5));
    let (store, _) =
        Store::open(Roots::single(Arc::new(MemFs::default())), StoreOptions::default(), clock.clone()).unwrap();
    let store = Arc::new(store);
    let cache = Arc::new(Cache::new(
        Arc::clone(&store),
        HostRules::default(),
        &Profiles::default(),
        Arc::new(NoEvents),
        ServeOptions::default(),
    ));
    Arc::new(Status::new(cache, store, clock, Arc::new(NoNames), 30_000))
}

fn api() -> AdminApi {
    AdminApi::new(status(), Duration::from_secs(1))
}

/// Status, headers and body of one request.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

fn get(api: &AdminApi, method: &str, target: &str) -> Answer {
    let req = RequestHead { method: method.into(), target: target.into(), minor_version: 1, headers: vec![] };
    let mut c = Capture::default();
    api.handle(&req, &mut ConnState::default(), &mut c).unwrap();
    Answer { status: c.status, headers: c.headers, body: String::from_utf8_lossy(&c.body).into_owned() }
}

/// A viewer that holds its stream open until let go.
struct Holding(Arc<(Mutex<bool>, std::sync::Condvar)>, Arc<std::sync::atomic::AtomicUsize>);

impl ResponseSink for Holding {
    fn send_head(&mut self, _: &ResponseHead<'_>) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_stream_head(&mut self, _: u16, _: &[(&str, &str)]) -> Result<(), SinkError> {
        // Counted once its stream is open.
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    fn send_body(&mut self, _: &[u8]) -> Result<(), SinkError> {
        let (gone, cv) = &*self.0;
        drop(cv.wait_while(gone.lock().unwrap(), |g| !*g).unwrap());
        Err(SinkError("closed".into()))
    }
}

#[test]
fn event_streams_leave_the_admin_port_free_for_the_rest() {
    use lanstash_adapter_admin_api::api::MAX_STREAMS;
    let a = Arc::new(api());
    let gone = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let open = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let viewers: Vec<_> = (0..MAX_STREAMS)
        .map(|_| {
            let (a, gone, open) = (Arc::clone(&a), Arc::clone(&gone), Arc::clone(&open));
            std::thread::spawn(move || {
                let req = RequestHead {
                    method: "GET".into(),
                    target: "/api/v1/events".into(),
                    minor_version: 1,
                    headers: vec![],
                };
                // Ends when the viewer goes (an error from its sink, as a closed page gives).
                let _ = a.handle(&req, &mut ConnState::default(), &mut Holding(gone, open));
            })
        })
        .collect();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while open.load(std::sync::atomic::Ordering::SeqCst) < MAX_STREAMS {
        assert!(std::time::Instant::now() < until, "the viewers' streams did not open");
        std::thread::yield_now();
    }
    // A further viewer, once the others hold every stream.
    let mut c = Capture::default();
    let req = RequestHead { method: "GET".into(), target: "/api/v1/events".into(), minor_version: 1, headers: vec![] };
    a.handle(&req, &mut ConnState::default(), &mut c).unwrap();
    assert_eq!(c.status, 503, "one stream too many");
    assert_eq!(get(&a, "GET", "/api/v1/version").status, 200, "the rest answers");
    *gone.0.lock().unwrap() = true;
    gone.1.notify_all();
    for v in viewers {
        v.join().unwrap();
    }
}

#[test]
fn a_page_elsewhere_cannot_restart_it_through_a_browser() {
    let restarted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&restarted);
    let a = api().with_stop(Arc::new(move || flag.store(true, std::sync::atomic::Ordering::SeqCst)));
    let headers = vec![("host".into(), "10.0.0.10:8081".into()), ("origin".into(), "http://evil.example".into())];
    let req = RequestHead { method: "POST".into(), target: "/shutdown".into(), minor_version: 1, headers };
    let mut c = Capture::default();
    a.handle(&req, &mut ConnState::default(), &mut c).unwrap();
    assert_eq!(c.status, 403);
    assert!(!restarted.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn the_api_answers_json_and_json_errors() {
    let a = api();
    let v = get(&a, "GET", "/api/v1/version");
    assert_eq!((v.status, v.body), (200, format!("{{\"version\":\"{VERSION}\"}}\n")));
    let s = get(&a, "GET", "/api/v1/status");
    assert_eq!(s.status, 200);
    assert!(s.headers.contains(&("Content-Type".into(), "application/json".into())));
    assert!(s.body.starts_with("{\"now_ms\":5,"), "{}", s.body);
    let m = get(&a, "DELETE", "/api/v1/status");
    assert_eq!((m.status, m.body.as_str()), (405, "{\"error\":\"only GET, HEAD\",\"kind\":\"method\"}\n"));
    assert!(m.headers.contains(&("Allow".into(), "GET, HEAD".into())));
    let n = get(&a, "GET", "/api/v1/prefill");
    assert_eq!((n.status, n.body.as_str()), (404, "{\"error\":\"not found\",\"kind\":\"not-found\"}\n"));
    let old = get(&a, "GET", "/nothing");
    assert_eq!((old.status, old.body.as_str()), (404, "not found\n"), "outside the API: text");
    let page = get(&a, "GET", "/");
    assert!(page.body.contains("new EventSource(\"api/v1/events\")"), "the page follows the API stream");
    assert_eq!(get(&a, "POST", "/api/v1/shutdown").status, 404, "no stop hook");
}

#[test]
fn further_counters_are_in_every_snapshot() {
    let a = api().with_counters("https", Arc::new(|| vec![("relayed", 3), ("bytes_up", 7)]));
    let s = get(&a, "GET", "/api/v1/status").body;
    assert!(s.contains("\"https\":{\"relayed\":3,\"bytes_up\":7}"), "{s}");
}

/// Records a streamed response; fails after `limit` bodies (the viewer leaves).
#[derive(Default)]
struct Viewer {
    stream_head: Option<(u16, Vec<(String, String)>)>,
    bodies: Vec<String>,
    limit: usize,
}

impl ResponseSink for Viewer {
    fn send_head(&mut self, _head: &ResponseHead<'_>) -> Result<(), SinkError> {
        panic!("an event stream has no length");
    }
    fn send_stream_head(&mut self, status: u16, headers: &[(&str, &str)]) -> Result<(), SinkError> {
        self.stream_head = Some((status, headers.iter().map(|(n, v)| ((*n).into(), (*v).into())).collect()));
        Ok(())
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        if self.bodies.len() >= self.limit {
            return Err(SinkError("closed".into()));
        }
        self.bodies.push(String::from_utf8(data.to_vec()).unwrap());
        Ok(())
    }
}

#[test]
fn events_stream_snapshots_until_the_viewer_leaves() {
    let page = AdminApi::new(status(), Duration::from_millis(100));
    let req = RequestHead { method: "GET".into(), target: "/api/v1/events".into(), minor_version: 1, headers: vec![] };
    let mut v = Viewer { limit: 3, ..Viewer::default() };
    page.handle(&req, &mut ConnState::default(), &mut v).unwrap();

    let (status, headers) = v.stream_head.unwrap();
    assert_eq!(status, 200);
    assert!(headers.contains(&("Content-Type".into(), "text/event-stream".into())));
    assert_eq!(v.bodies[0], "retry: 2000\n\n");
    for e in &v.bodies[1..] {
        assert!(e.starts_with("data: {\"now_ms\":5,") && e.ends_with("}\n\n"), "{e}");
        assert!(!e[..e.len() - 2].contains('\n'), "one line per event");
    }
    assert_eq!(v.bodies.len(), 3);
}

/// Headers of numeric ids at a fixed address, as Steam's are.
struct Headers;

impl ArtSource for Headers {
    fn locate(&self, id: &str, kind: ArtKind) -> Result<Source, ArtError> {
        let numeric = !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit());
        match kind {
            ArtKind::Header if numeric => {
                Ok(Source { host: "cdn".into(), port: None, target: format!("/steam/apps/{id}/header.jpg") })
            }
            _ => Err(ArtError::Unknown),
        }
    }
}

#[test]
fn art_is_served_with_its_type_or_a_json_error() {
    let cdn = FakeUpstream::default();
    let jpg = FakeResponse::ok(b"\xFF\xD8\xFF\xE0JPG", &[("content-type", "image/jpeg")]);
    cdn.route("cdn", "/steam/apps/1422450/header.jpg", jpg);
    cdn.route("cdn", "/steam/apps/7/header.jpg", FakeResponse { status: 404, ..FakeResponse::ok(b"", &[]) });
    let art = Art::new(Arc::new(cdn), Arc::new(FixedClock(5)), ArtOptions::default());
    let a = api().with_art(Arc::new(art.with_source("steam", Arc::new(Headers))));
    let img = get(&a, "GET", "/api/v1/art/steam/1422450");
    assert_eq!((img.status, img.body.ends_with("JPG")), (200, true));
    for h in [
        ("Content-Type", "image/jpeg"),
        ("Cache-Control", "public, max-age=86400"),
        ("X-Content-Type-Options", "nosniff"),
    ] {
        assert!(img.headers.contains(&(h.0.into(), h.1.into())), "{h:?} in {:?}", img.headers);
    }
    let kind = |target: &str| {
        let p = get(&a, "GET", target);
        (p.status, p.body.split("\"kind\":").nth(1).map(|k| k.trim_end().trim_end_matches('}').to_owned()))
    };
    let none = (404, Some("\"not-found\"".to_owned()));
    assert_eq!(kind("/api/v1/art/steam/7"), none, "the CDN has none");
    assert_eq!(kind("/api/v1/art/blizzard/osi"), none, "no source for the platform");
    assert_eq!(kind("/api/v1/art/steam"), none, "no id");
    assert_eq!(kind("/api/v1/art/steam/1422450/poster"), none, "unknown kind");
    assert_eq!(kind("/api/v1/art/steam/1422450/icon"), none, "no Steam icons");
    assert_eq!(kind("/api/v1/art/steam/1422450/header/x"), none, "too many parts");
    assert_eq!(get(&a, "GET", "/api/v1/art/steam/1422450/header").status, 200);
    assert_eq!(kind("/api/v1/art/steam/5"), (502, Some("\"upstream\"".to_owned())), "the CDN is unreachable");
    assert_eq!(get(&api(), "GET", "/api/v1/art/steam/1422450").status, 404, "art not enabled");
}

/// A prefill that records what it was asked and answers `start` with `outcome`.
struct Jobs {
    outcome: Result<(), StartError>,
    started_ms: u64,
    asked: Mutex<Vec<(String, bool, PrefillFrom)>>,
    stopped: Mutex<bool>,
    unqueued: Mutex<Vec<String>>,
}

impl Jobs {
    fn new(outcome: Result<(), StartError>, started_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            outcome,
            started_ms,
            asked: Mutex::default(),
            stopped: Mutex::default(),
            unqueued: Mutex::default(),
        })
    }
}

impl Prefill for Jobs {
    fn start(&self, game: &str, prerelease: bool, from: PrefillFrom) -> Result<(), StartError> {
        self.asked.lock().unwrap().push((game.into(), prerelease, from));
        self.outcome.clone()
    }
    fn stop(&self) {
        *self.stopped.lock().unwrap() = true;
    }
    fn unqueue(&self, game: &str) -> bool {
        self.unqueued.lock().unwrap().push(game.to_owned());
        game == "waiting"
    }
    fn status(&self) -> Option<PrefillStatus> {
        let asked = self.asked.lock().unwrap();
        let (game, prerelease, from) = asked.last()?.clone();
        Some(PrefillStatus::started(&game, prerelease, from, self.started_ms))
    }
}

/// A platform whose views echo what they were asked; game `gone` is unavailable.
#[derive(Debug)]
struct Echo {
    id: &'static str,
    views: bool,
    jobs: Option<Arc<Jobs>>,
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Jobs")
    }
}

impl Platform for Echo {
    fn id(&self) -> &str {
        self.id
    }
    fn builds(&self) -> Option<View> {
        self.views.then(|| Ok(format!("{{\"builds\":\"{}\"}}\n", self.id)))
    }
    fn catalog(&self) -> Option<View> {
        self.views.then(|| Err(ViewError::Unavailable("no catalog yet".into())))
    }
    fn game(&self, game: &str) -> Option<View> {
        let missing = Err(ViewError::NotFound(format!("no game {game}")));
        self.views.then(|| if game == "gone" { missing } else { Ok(format!("{{\"game\":\"{game}\"}}\n")) })
    }
    fn nginx(&self, game: &str) -> Option<View> {
        self.views.then(|| Ok(format!("{{\"nginx\":\"{game}\"}}\n")))
    }
    fn prefill(&self) -> Option<Arc<dyn Prefill>> {
        self.jobs.clone().map(|j| j as Arc<dyn Prefill>)
    }
}

#[test]
fn platform_paths_are_answered_by_the_platform_they_name() {
    let a = api().with_platforms(vec![
        Arc::new(Echo { id: "blizzard", views: true, jobs: None }),
        Arc::new(Echo { id: "steam", views: false, jobs: None }),
    ]);
    let body = |target: &str| {
        let r = get(&a, "GET", target);
        (r.status, r.body)
    };
    assert_eq!(body("/api/v1/builds/blizzard"), (200, "{\"builds\":\"blizzard\"}\n".into()));
    assert_eq!(body("/api/v1/builds/steam").0, 404, "the platform has no builds");
    assert_eq!(body("/api/v1/builds/epic").0, 404, "no such platform");
    assert_eq!(body("/api/v1/builds/blizzard/x").0, 404);
    assert_eq!(
        body("/api/v1/catalog/blizzard"),
        (503, "{\"error\":\"no catalog yet\",\"kind\":\"unavailable\"}\n".into())
    );
    assert_eq!(body("/api/v1/games/blizzard/osi"), (200, "{\"game\":\"osi\"}\n".into()));
    assert_eq!(body("/api/v1/games/blizzard/osi/nginx"), (200, "{\"nginx\":\"osi\"}\n".into()));
    assert_eq!(
        body("/api/v1/games/blizzard/gone"),
        (404, "{\"error\":\"no game gone\",\"kind\":\"not-found\"}\n".into())
    );
    let no_game = (404, "{\"error\":\"no such game\",\"kind\":\"not-found\"}\n".to_owned());
    assert_eq!(body("/api/v1/games/blizzard/a%2Fb"), no_game, "not a game directory");
    assert_eq!(body("/api/v1/games/blizzard/a.b/nginx"), no_game);
    assert_eq!(body("/api/v1/games/blizzard"), no_game);
    assert_eq!(body("/api/v1/games/steam/7"), no_game, "the platform has no game view");
}
