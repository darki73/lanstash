//! Tests for `lanstash_app::serve::tally`: how a client's response body went, counted (sent by the
//! file itself or copied, and whether the client went away before its end).

use super::*;
use lanstash_app::serve::{ResponseHead, ResponseSink, SinkError};
use lanstash_ports::storage::FileRead;

/// A sink whose connection sends a stored body from the file itself (as `sendfile` does).
#[derive(Default)]
struct FromFile {
    sent: u64,
}

impl ResponseSink for FromFile {
    fn send_head(&mut self, _: &ResponseHead<'_>) -> Result<(), SinkError> {
        Ok(())
    }
    fn send_body(&mut self, data: &[u8]) -> Result<(), SinkError> {
        self.sent += data.len() as u64;
        Ok(())
    }
    fn send_file(&mut self, _: &dyn FileRead, _: u64, len: u64) -> Result<bool, SinkError> {
        self.sent += len;
        Ok(true)
    }
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
    fn send_file(&mut self, _: &dyn FileRead, _: u64, _: u64) -> Result<bool, SinkError> {
        Err(SinkError("connection reset by peer".into()))
    }
}

fn n(a: &std::sync::atomic::AtomicU64) -> u64 {
    a.load(Ordering::Relaxed)
}

#[test]
fn bodies_count_as_sent_from_the_file_or_copied() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(300), &[]));
    let get = request("GET", BNET, BNET_PATH, &[]);
    // A miss: the download's bytes are copied to the client.
    r.cache.handle(&get, &mut ConnState::default(), &mut Capture::default()).unwrap();
    r.settle();
    let k = &r.cache.counters;
    assert_eq!((n(&k.copied_bytes), n(&k.sendfile_bytes)), (300, 0));
    // A hit on a connection that sends from the file: none copied.
    let mut sink = FromFile::default();
    r.cache.handle(&get, &mut ConnState::default(), &mut sink).unwrap();
    assert_eq!(sink.sent, 300);
    assert_eq!((n(&k.copied_bytes), n(&k.sendfile_bytes)), (300, 300));
    assert_eq!((n(&k.aborted), n(&k.errors)), (0, 0));
}

#[test]
fn a_client_gone_mid_response_is_an_abort_not_an_error() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(300), &[]));
    let get = request("GET", BNET, BNET_PATH, &[]);
    assert!(r.cache.handle(&get, &mut ConnState::default(), &mut Gone).is_err());
    r.settle();
    assert!(r.cache.handle(&get, &mut ConnState::default(), &mut Gone).is_err(), "and from the stash");
    let k = &r.cache.counters;
    assert_eq!((n(&k.aborted), n(&k.errors)), (2, 0));
}
