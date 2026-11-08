//! Small whole-file GETs from origin servers.

use lanstash_app::origin::{OriginError, get, get_range};
use lanstash_app::test_support::{FakeResponse, FakeUpstream};
use lanstash_ports::upstream::Source;

fn src(target: &str) -> Source {
    Source { host: "cdn".into(), port: None, target: target.into() }
}

#[test]
fn reads_whole_bodies_up_to_a_limit() {
    let up = FakeUpstream::default();
    up.route("cdn", "/a", FakeResponse::ok(b"hello world", &[]));
    up.route("cdn", "/gone", FakeResponse { status: 410, ..FakeResponse::ok(b"", &[]) });
    up.route("cdn", "/err", FakeResponse { status: 500, ..FakeResponse::ok(b"", &[]) });
    up.route("cdn", "/chunked", FakeResponse { chunked: true, ..FakeResponse::ok(b"0123456789", &[]) });
    assert_eq!(get(&up, &src("/a"), 100).unwrap(), b"hello world");
    assert_eq!(get(&up, &src("/gone"), 100), Err(OriginError::Missing));
    assert_eq!(get(&up, &src("/err"), 100), Err(OriginError::Failed("cdn/err: status 500".into())));
    assert_eq!(get(&up, &src("/a"), 5), Err(OriginError::Failed("cdn/a: over 5 bytes".into())), "by its length");
    assert_eq!(get(&up, &src("/chunked"), 5), Err(OriginError::Failed("cdn/chunked: over 5 bytes".into())));
    assert!(matches!(get(&up, &src("/unrouted"), 5), Err(OriginError::Failed(_))));
    assert_eq!(up.seen.lock().unwrap()[0].method, "GET");
}

#[test]
fn reads_a_range_only_when_it_comes_back_as_one() {
    let up = FakeUpstream::default();
    up.route("cdn", "/archive", FakeResponse::ranged(b"0123456789", &[]));
    up.route("cdn", "/whole", FakeResponse::ok(b"0123456789", &[]));
    assert_eq!(get_range(&up, &src("/archive"), Some((2, 5)), 100).unwrap(), b"234");
    let asked = up.seen.lock().unwrap()[0].headers.clone();
    assert!(asked.iter().any(|(n, v)| n.eq_ignore_ascii_case("range") && v == "bytes=2-4"), "{asked:?}");
    assert_eq!(get_range(&up, &src("/archive"), None, 100).unwrap(), b"0123456789", "no range: the whole file");
    assert!(
        matches!(get_range(&up, &src("/whole"), Some((2, 5)), 100), Err(OriginError::Failed(_))),
        "a server that ignores the range would send the whole file"
    );
}
