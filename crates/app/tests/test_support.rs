//! The origin and sink doubles behave like the things they stand in for.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use lanstash_app::serve::{ResponseHead, ResponseSink};
use lanstash_app::test_support::{Capture, FakeResponse, FakeUpstream};
use lanstash_ports::upstream::{Upstream, UpstreamRequest};

fn get(up: &FakeUpstream, target: &str, headers: &[(&str, &str)]) -> Result<(u16, Vec<u8>), String> {
    let req = UpstreamRequest { method: "GET", host: "h", port: None, target, headers };
    let mut r = up.fetch(&req).map_err(|e| e.message)?;
    let status = r.head().status;
    let mut body = Vec::new();
    let mut buf = [0u8; 64];
    loop {
        let n = r.read(&mut buf).map_err(|e| e.message)?;
        if n == 0 {
            return Ok((status, body));
        }
        body.extend_from_slice(&buf[..n]);
    }
}

#[test]
fn fake_upstream_routes_records_and_refuses_the_rest() {
    let up = FakeUpstream::default();
    up.route("h", "/a", FakeResponse::ok(b"hello, world", &[("X-One", "1")]));
    assert_eq!(get(&up, "/a", &[]).unwrap(), (200, b"hello, world".to_vec()));
    assert_eq!(get(&up, "/b", &[]).unwrap_err(), "connection refused");
    assert_eq!(up.calls(), 2);
    assert_eq!(up.seen.lock().unwrap()[1].target, "/b");
}

#[test]
fn fake_upstream_honours_ranges_only_when_asked() {
    let up = FakeUpstream::default();
    up.route("h", "/r", FakeResponse::ranged(b"0123456789", &[]));
    up.route("h", "/w", FakeResponse::ok(b"0123456789", &[]));
    assert_eq!(get(&up, "/r", &[("Range", "bytes=2-4")]).unwrap(), (206, b"234".to_vec()));
    assert_eq!(get(&up, "/r", &[("Range", "bytes=20-30")]).unwrap(), (416, Vec::new()));
    assert_eq!(get(&up, "/w", &[("Range", "bytes=2-4")]).unwrap(), (200, b"0123456789".to_vec()));
}

#[test]
fn fake_upstream_fails_mid_body_the_given_number_of_times() {
    let up = FakeUpstream::default();
    let r = FakeResponse {
        fail_after: Some(3),
        fail_times: Some(Arc::new(AtomicUsize::new(1))),
        ..FakeResponse::ok(b"0123456789", &[])
    };
    up.route("h", "/f", r);
    assert_eq!(get(&up, "/f", &[]).unwrap_err(), "connection reset");
    assert_eq!(get(&up, "/f", &[]).unwrap(), (200, b"0123456789".to_vec()));
}

#[test]
fn capture_keeps_the_head_and_body() {
    let mut c = Capture::default();
    let head = ResponseHead {
        status: 206,
        reason: "Partial Content",
        headers: &[("Content-Range", "bytes 0-1/2")],
        content_length: 2,
    };
    c.send_head(&head).unwrap();
    c.send_body(b"ab").unwrap();
    assert_eq!((c.status, c.content_length, c.body.as_slice()), (206, 2, b"ab".as_slice()));
    assert_eq!(c.header("content-range"), Some("bytes 0-1/2"));
}
