//! Shutdown on the admin API (`/api/v1/shutdown` and the older `/shutdown`), and `lanstash_adapter_admin_api::client`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lanstash_adapter_admin_api::api::AdminApi;
use lanstash_adapter_admin_api::client::{is_up, local_addr, request, request_api, status};
use lanstash_adapter_http::server::{HttpServer, ServerConfig};
use lanstash_app::serve::{Cache, ConnState, RequestHandler, ResponseHead, ResponseSink, ServeOptions, SinkError};
use lanstash_app::status::Status;
use lanstash_domain::http::{Limits, RequestHead};
use lanstash_domain::service::Profiles;
use lanstash_domain::service::hosts::HostRules;
use lanstash_ports::clock::FixedClock;
use lanstash_ports::events::NoEvents;
use lanstash_ports::names::NoNames;
use lanstash_store::store::{Roots, Store, StoreOptions};
use lanstash_store::test_support::MemFs;

fn page() -> AdminApi {
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
    AdminApi::new(Arc::new(Status::new(cache, store, clock, Arc::new(NoNames), 30_000)), Duration::from_secs(1))
}

fn serve(page: AdminApi) -> SocketAddr {
    serve_handler(Arc::new(page))
}

fn serve_handler(handler: Arc<dyn RequestHandler>) -> SocketAddr {
    let server = HttpServer::bind(ServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        max_connections: 4,
        idle_timeout: Duration::from_secs(5),
        head_timeout: Duration::from_secs(5),
        max_per_client: 4,
        write_timeout: Duration::from_secs(5),
        limits: Limits::default(),
        thread_stack_bytes: 256 * 1024,
        zero_copy: true,
    })
    .unwrap();
    let addr = server.local_addr().unwrap();
    std::thread::spawn(move || server.run(&handler));
    addr
}

#[test]
fn shutdown_from_this_machine_runs_the_hook() {
    let stopped = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopped);
    let addr = serve(page().with_stop(Arc::new(move || flag.store(true, Ordering::SeqCst))));
    assert!(is_up(addr, Duration::from_secs(1)));
    let t = Duration::from_secs(5);
    assert_eq!(request(addr, "GET", "/shutdown", t).unwrap().0, 405, "GET does not stop");
    assert_eq!(request(addr, "POST", "/shutdown", t).unwrap(), (202, "stopping\n".to_owned()));
    for _ in 0..100 {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(stopped.load(Ordering::SeqCst));
}

#[test]
fn shutdown_through_the_api_answers_json() {
    let stopped = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopped);
    let addr = serve(page().with_stop(Arc::new(move || flag.store(true, Ordering::SeqCst))));
    let t = Duration::from_secs(5);
    let (status, body) = request(addr, "GET", "/api/v1/shutdown", t).unwrap();
    assert_eq!((status, body.as_str()), (405, "{\"error\":\"only POST\",\"kind\":\"method\"}\n"));
    assert_eq!(request(addr, "POST", "/api/v1/shutdown", t).unwrap(), (202, "{\"stopping\":true}\n".to_owned()));
    for _ in 0..100 {
        if stopped.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(stopped.load(Ordering::SeqCst));
}

#[test]
fn the_client_reads_the_status_through_the_api() {
    let addr = serve(page());
    let (code, body) = status(addr, Duration::from_secs(5)).unwrap();
    assert_eq!(code, 200);
    assert!(body.starts_with("{\"now_ms\":5,"), "{body}");
    let (code, body) = request(addr, "GET", "/api/v1/nothing", Duration::from_secs(5)).unwrap();
    assert_eq!((code, body.as_str()), (404, "{\"error\":\"not found\",\"kind\":\"not-found\"}\n"));
}

/// An older lanstash, answering as the one before the API did: its paths,
/// 404 for any other GET, 405 for any other POST (a stop through the API
/// path gets `405 GET only`, so the client must fall back on 405 too).
struct OldLanstash;

impl RequestHandler for OldLanstash {
    fn handle(&self, req: &RequestHead, _: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        let (status, body) = match (req.method.as_str(), req.target.as_str()) {
            ("GET", "/status.json") => (200, "{\"old\":1}"),
            ("POST", "/shutdown") => (202, "stopping\n"),
            ("GET", _) => (404, "not found\n"),
            _ => (405, "GET only\n"),
        };
        sink.send_head(&ResponseHead {
            status,
            reason: "",
            headers: &[("Content-Type", "text/plain")],
            content_length: body.len() as u64,
        })?;
        sink.send_body(body.as_bytes())
    }
}

#[test]
fn the_client_falls_back_to_the_paths_of_an_older_lanstash() {
    let addr = serve_handler(Arc::new(OldLanstash));
    assert_eq!(status(addr, Duration::from_secs(5)).unwrap(), (200, "{\"old\":1}".to_owned()));
    let t = Duration::from_secs(5);
    let stop = request_api(addr, "POST", "/api/v1/shutdown", "/shutdown", t).unwrap();
    assert_eq!(stop, (202, "stopping\n".to_owned()), "405 on the API path: the older path");
    let both = request_api(addr, "GET", "/api/v1/x", "/y", t).unwrap();
    assert_eq!(both.0, 404, "both missing: the older path's answer");
}

#[test]
fn shutdown_without_a_hook_does_not_exist() {
    let addr = serve(page());
    assert_eq!(request(addr, "POST", "/shutdown", Duration::from_secs(5)).unwrap().0, 404);
}

/// Records the status of a plain response.
#[derive(Default)]
struct Status0(Option<u16>);

impl ResponseSink for Status0 {
    fn send_head(&mut self, head: &ResponseHead<'_>) -> Result<(), SinkError> {
        self.0 = Some(head.status);
        Ok(())
    }
    fn send_body(&mut self, _data: &[u8]) -> Result<(), SinkError> {
        Ok(())
    }
}

#[test]
fn shutdown_from_elsewhere_is_refused() {
    let page = page().with_stop(Arc::new(|| panic!("must not stop")));
    let req = RequestHead { method: "POST".into(), target: "/shutdown".into(), minor_version: 1, headers: vec![] };
    let mut sink = Status0::default();
    page.handle(&req, &mut ConnState::new(false), &mut sink).unwrap();
    assert_eq!(sink.0, Some(403));
}

#[test]
fn wildcard_admin_addresses_are_reached_through_loopback() {
    assert_eq!(local_addr("0.0.0.0:8081".parse().unwrap()), "127.0.0.1:8081".parse().unwrap());
    assert_eq!(local_addr("[::]:8081".parse().unwrap()), "[::1]:8081".parse().unwrap());
    assert_eq!(local_addr("10.0.0.5:8081".parse().unwrap()), "10.0.0.5:8081".parse().unwrap());
}

#[test]
fn a_status_answer_gives_the_stores_figures() {
    use lanstash_adapter_admin_api::client::{StoreFigures, store_figures};
    let body = r#"{"now_ms":1,"counters":{"verify":{"problems":2}},"usage":[
        {"service":"steam","game":"7","bytes":1000},{"service":"blizzard","game":"wow","bytes":500}]}"#;
    assert_eq!(store_figures(body), Some(StoreFigures { games: 2, bytes: 1_500, problems: 2 }));
    let quiet = r#"{"counters":{},"usage":[]}"#;
    assert_eq!(store_figures(quiet), Some(StoreFigures { games: 0, bytes: 0, problems: 0 }), "no background check");
    assert_eq!(store_figures("not json"), None);
    assert_eq!(store_figures("{}"), None, "no usage: not a status answer");
}
