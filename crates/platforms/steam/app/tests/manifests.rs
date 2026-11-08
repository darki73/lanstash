//! Tests for `lanstash_steam_app::manifests`, against a scripted account and a fake CDN.

mod support;

use lanstash_steam_app::manifests::{USER_AGENT, fetch, servers};
use support::account::{CDN, services};

#[test]
fn a_manifest_comes_from_a_listed_server_with_its_request_code() {
    let (s, up) = services(true);
    let (mut c, on) = s.session("someone").unwrap();
    let list = servers(&mut c, on.cell_id).unwrap();
    assert_eq!(list.iter().map(|s| s.host.as_str()).collect::<Vec<_>>(), [CDN]);
    let m = fetch(&mut c, up.as_ref(), &list, (100, 101, 11)).unwrap();
    assert_eq!((m.depot, m.gid, m.chunks().len(), m.download()), (101, 11, 2, 300));
    let seen = up.seen.lock().unwrap();
    assert_eq!(seen[0].target, "/depot/101/manifest/11/5/1234");
    assert!(seen[0].headers.iter().any(|(n, v)| n == "User-Agent" && v == USER_AGENT));
}

#[test]
fn a_manifest_of_another_depot_or_build_is_not_taken_for_the_one_asked() {
    use lanstash_app::test_support::FakeResponse;
    let (s, up) = services(true);
    let (mut c, on) = s.session("someone").unwrap();
    let list = servers(&mut c, on.cell_id).unwrap();
    // The server answers the path asked for with another build's manifest.
    let other = support::account::manifest(101, 99, &[(1, 100)]);
    up.route(CDN, "/depot/101/manifest/11/5/1234", FakeResponse::ok(&other, &[]));
    let e = fetch(&mut c, up.as_ref(), &list, (100, 101, 11)).unwrap_err();
    assert!(e.contains("got manifest 99 of depot 101, not 11"), "{e}");
}

#[test]
fn a_manifest_no_server_has_is_an_error() {
    let (s, up) = services(true);
    let (mut c, on) = s.session("someone").unwrap();
    let list = servers(&mut c, on.cell_id).unwrap();
    let e = fetch(&mut c, up.as_ref(), &list, (100, 101, 99)).unwrap_err();
    assert!(e.starts_with("depot 101 from cdn.example: "), "{e}");
}
