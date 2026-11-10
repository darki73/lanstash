//! Tests for `lanstash_adapter_admin_api::api::version`: the running version, and a newer release
//! when one is known.

use super::*;

#[test]
fn the_version_says_a_newer_release_only_when_one_is_known() {
    let v = get(&api(), "GET", "/api/v1/version");
    assert_eq!(v.body, format!("{{\"version\":\"{VERSION}\"}}\n"), "no release watch: no latest");
    let newer = api().with_releases(Arc::new(|| Some("9.9.9".into())));
    assert_eq!(
        get(&newer, "GET", "/api/v1/version").body,
        format!("{{\"version\":\"{VERSION}\",\"latest\":\"9.9.9\"}}\n")
    );
    let none = api().with_releases(Arc::new(|| None));
    assert_eq!(get(&none, "GET", "/api/v1/version").body, format!("{{\"version\":\"{VERSION}\"}}\n"));
}
