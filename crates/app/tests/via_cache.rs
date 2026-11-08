//! GETs through a request handler, as a client would send them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lanstash_app::serve::{ConnState, RequestHandler, ResponseHead, ResponseSink, SinkError};
use lanstash_app::via_cache::{Get, USER_AGENT, Watch, get};
use lanstash_domain::http::RequestHead;

/// Answers with `status` and a ten-byte body; remembers the requests.
struct Ten {
    status: u16,
    seen: Mutex<Vec<RequestHead>>,
}

impl Ten {
    fn new(status: u16) -> Self {
        Self { status, seen: Mutex::new(Vec::new()) }
    }
}

impl RequestHandler for Ten {
    fn handle(&self, req: &RequestHead, _conn: &mut ConnState, sink: &mut dyn ResponseSink) -> Result<(), SinkError> {
        self.seen.lock().unwrap().push(req.clone());
        sink.send_head(&ResponseHead { status: self.status, reason: "", headers: &[], content_length: 10 })?;
        sink.send_body(b"01234")?;
        sink.send_body(b"56789")
    }
}

fn whole(target: &str) -> Get<'_> {
    Get { host: "cdn", target, range: None, keep: Some(100) }
}

#[test]
fn sends_a_client_request_and_keeps_the_body() {
    let h = Ten::new(200);
    assert_eq!(get(&h, whole("/a"), Watch::default()).unwrap(), Some(b"0123456789".to_vec()));
    let ranged = Get { range: Some((5, 10)), keep: None, ..whole("/a") };
    assert_eq!(get(&h, ranged, Watch::default()).unwrap(), Some(Vec::new()), "the body is dropped unless kept");
    let seen = h.seen.lock().unwrap();
    assert_eq!(seen[0].header("host"), Some("cdn"));
    assert_eq!(seen[0].header("user-agent"), Some(USER_AGENT));
    assert_eq!(seen[1].header("range"), Some("bytes=5-9"));
}

#[test]
fn missing_files_are_none_and_other_answers_errors() {
    assert_eq!(get(&Ten::new(404), whole("/a"), Watch::default()), Ok(None));
    assert_eq!(get(&Ten::new(410), whole("/a"), Watch::default()), Ok(None));
    assert_eq!(get(&Ten::new(502), whole("/a"), Watch::default()), Err("/a: status 502".into()));
    let small = Get { keep: Some(7), ..whole("/a") };
    assert_eq!(get(&Ten::new(200), small, Watch::default()), Err("/a: over 7 bytes".into()));
}

#[test]
fn counts_bytes_and_stops_when_asked() {
    let (bytes, mine, stop) = (AtomicU64::new(0), AtomicU64::new(0), AtomicBool::new(false));
    let watch = Watch { bytes: Some(&bytes), mine: Some(&mine), stop: Some(&stop) };
    get(&Ten::new(200), whole("/a"), watch).unwrap();
    assert_eq!((bytes.load(Ordering::Relaxed), mine.load(Ordering::Relaxed)), (10, 10));
    stop.store(true, Ordering::Relaxed);
    assert_eq!(get(&Ten::new(200), whole("/a"), watch), Err("/a: stopped".into()));
}
