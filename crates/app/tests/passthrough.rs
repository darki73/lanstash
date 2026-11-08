//! Requests that are not cached are relayed as they are.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use lanstash_app::passthrough::Passthrough;
use lanstash_app::serve::{ResponseHead, ResponseSink, SinkError};
use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream};
use lanstash_domain::http::RequestHead;

fn req(method: &str, target: &str) -> RequestHead {
    let headers =
        vec![("host".into(), "cdn".into()), ("user-agent".into(), "test".into()), ("x-other".into(), "1".into())];
    RequestHead { method: method.into(), target: target.into(), minor_version: 1, headers }
}

#[test]
fn relays_the_answer_and_forwards_only_known_headers() {
    let up = Arc::new(FakeUpstream::default());
    up.route("cdn", "/v", FakeResponse::ok(b"version 7", &[("content-type", "text/plain")]));
    let pass = Passthrough::new(up.clone(), 1 << 20, 4096);
    let mut c = Capture::default();
    pass.relay(&req("GET", "/v"), "cdn", &mut c).unwrap();
    assert_eq!((c.status, c.body.as_slice()), (200, b"version 7".as_slice()));
    assert_eq!(c.header("x-lanstash"), Some("PASS"));
    assert_eq!(c.header("content-type"), Some("text/plain"));
    let seen = up.seen.lock().unwrap()[0].headers.clone();
    assert!(seen.iter().any(|(n, v)| n == "user-agent" && v == "test"));
    assert!(!seen.iter().any(|(n, _)| n == "x-other"));
    assert!(seen.iter().any(|(n, _)| n == "X-LanStash-Via"), "marked so a loop is seen");
    assert_eq!(pass.counters.bytes.load(Ordering::Relaxed), 9);
}

#[test]
fn a_failed_origin_is_a_bad_gateway() {
    let pass = Passthrough::new(Arc::new(FakeUpstream::default()), 1 << 20, 4096);
    let mut c = Capture::default();
    pass.relay(&req("GET", "/nowhere"), "cdn", &mut c).unwrap();
    assert_eq!(c.status, 502);
    assert_eq!(c.header("x-lanstash"), Some("UPSTREAM-ERROR"));
    assert_eq!(pass.counters.errors.load(Ordering::Relaxed), 1);
}

/// A client that hung up: the head goes, the body does not.
struct Gone;

impl ResponseSink for Gone {
    fn send_head(&mut self, _: &ResponseHead<'_>) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_body(&mut self, _: &[u8]) -> Result<(), SinkError> {
        Err(SinkError("connection reset by peer".into()))
    }
}

#[test]
fn counts_a_body_cut_short_too_large_or_left_by_the_client() {
    let up = Arc::new(FakeUpstream::default());
    up.route("cdn", "/cut", FakeResponse { fail_after: Some(3), ..FakeResponse::ok(b"version 7", &[]) });
    up.route("cdn", "/big", FakeResponse { chunked: true, ..FakeResponse::ok(&[7; 64], &[]) });
    up.route("cdn", "/v", FakeResponse::ok(b"version 7", &[]));
    let pass = Passthrough::new(up, 16, 4096);
    assert!(pass.relay(&req("GET", "/cut"), "cdn", &mut Capture::default()).is_err());
    let mut c = Capture::default();
    pass.relay(&req("GET", "/big"), "cdn", &mut c).unwrap();
    assert_eq!(c.status, 502, "no length and past what is buffered");
    assert!(pass.relay(&req("GET", "/v"), "cdn", &mut Gone).is_err());
    let n = |a: &std::sync::atomic::AtomicU64| a.load(Ordering::Relaxed);
    let k = &pass.counters;
    assert_eq!((n(&k.errors), n(&k.too_large), n(&k.aborted), n(&k.timeouts)), (1, 1, 1, 0));
}

#[test]
fn head_requests_get_no_body() {
    let up = Arc::new(FakeUpstream::default());
    up.route("cdn", "/v", FakeResponse::ok(b"version 7", &[]));
    let pass = Passthrough::new(up, 1 << 20, 4096);
    let mut c = Capture::default();
    pass.relay(&req("HEAD", "/v"), "cdn", &mut c).unwrap();
    assert_eq!(c.status, 200);
    assert!(c.body.is_empty());
}
