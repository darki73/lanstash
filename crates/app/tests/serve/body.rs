//! Tests for `lanstash_app::serve::body`: walking a range over stored parts
//! and downloads, and retrying a failed download.

use super::*;

#[test]
fn a_small_range_fetches_no_more_than_asked() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    let c = r.get(BNET, BNET_PATH, Some("bytes=500-509"));
    assert_eq!(c.body, &data[500..510]);
    r.settle();
    assert_eq!(r.ranges_seen(), vec![Some("bytes=500-509".to_owned())], "not a whole piece");
}

#[test]
fn a_larger_range_fetches_only_what_is_missing() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    r.get(BNET, BNET_PATH, Some("bytes=0-127"));
    r.settle();
    let before = r.up.calls();

    let c = r.get(BNET, BNET_PATH, Some("bytes=0-383"));
    assert_eq!(c.body, &data[..384]);
    assert_eq!(c.header("x-lanstash"), Some("PARTIAL"), "0..128 was stored already");
    assert_eq!(r.cache.counters.partial.load(std::sync::atomic::Ordering::Relaxed), 1);
    r.settle();
    let mut new: Vec<_> = r.ranges_seen()[before..].to_vec();
    new.sort(); // the pieces are fetched in parallel
    assert_eq!(new, vec![Some("bytes=128-255".to_owned()), Some("bytes=256-383".to_owned())]);
}

#[test]
fn whole_object_in_pieces_survives_restart() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    let c = r.get(BNET, BNET_PATH, None);
    assert_eq!((c.status, c.content_length), (200, 1000));
    assert_eq!(c.body, data);
    r.settle();

    let again = rig_on(r.fs.clone(), Arc::new(FakeUpstream::default()), fetch_opts());
    let c = again.get(BNET, BNET_PATH, None);
    assert_eq!((c.status, c.header("x-lanstash")), (200, Some("HIT")));
    assert_eq!(c.body, data);
    let c = again.get(BNET, BNET_PATH, Some("bytes=-10"));
    assert_eq!(c.body, &data[990..]);
    assert_eq!(again.up.calls(), 0);
}

#[test]
fn origin_that_ignores_range_still_gives_the_right_slice() {
    let r = rig();
    let data = body(600);
    r.up.route(BNET, BNET_PATH, FakeResponse::ok(&data, &[]));
    let c = r.get(BNET, BNET_PATH, Some("bytes=200-209"));
    assert_eq!((c.status, c.header("content-range")), (206, Some("bytes 200-209/600")));
    assert_eq!(c.body, &data[200..210]);
    r.settle();
    let c = r.get(BNET, BNET_PATH, None);
    assert_eq!(c.body, data);
}

#[test]
fn failed_download_is_retried_from_where_it_stopped() {
    let r = rig();
    let data = body(1000);
    let mut resp = FakeResponse::ranged(&data, &[]);
    resp.fail_after = Some(40);
    resp.fail_times = Some(Arc::new(AtomicUsize::new(2)));
    r.up.route(BNET, BNET_PATH, resp);
    let c = r.get(BNET, BNET_PATH, Some("bytes=0-499"));
    assert_eq!(c.body, &data[..500]);
    assert!(r.cache.counters.retries.load(Ordering::Relaxed) >= 1);
}

#[test]
fn an_error_answer_for_a_piece_fails_the_request_after_the_retries() {
    let r = rig();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&body(1000), &[]));
    r.get(BNET, BNET_PATH, Some("bytes=0-127"));
    r.settle();
    r.up.route(BNET, BNET_PATH, FakeResponse { status: 503, ..FakeResponse::ok(b"busy", &[]) });
    let before = r.up.calls();

    let (res, c) = r.send(&request("GET", BNET, BNET_PATH, &[]));
    assert!(res.is_err(), "the body cannot be completed");
    assert_eq!(c.body, &body(1000)[..128], "only the stored part, never the error page");
    r.settle();
    assert!(r.up.calls() - before <= 4 * 2, "bounded by the retries: {}", r.up.calls() - before);
}

#[test]
fn an_error_answer_for_a_piece_is_never_served_as_object_bytes() {
    let r = rig();
    let data = body(1000);
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    r.get(BNET, BNET_PATH, Some("bytes=0-127"));
    r.settle();
    r.up.route(BNET, BNET_PATH, FakeResponse { status: 404, ..FakeResponse::ok(b"gone", &[]) });
    let (_, c) = r.send(&request("GET", BNET, BNET_PATH, &[("range", "bytes=0-299")]));
    assert!(!c.body.windows(4).any(|w| w == b"gone"));
    r.settle();
    r.up.route(BNET, BNET_PATH, FakeResponse::ranged(&data, &[]));
    assert_eq!(r.get(BNET, BNET_PATH, None).body, data, "nothing wrong was stored");
}
