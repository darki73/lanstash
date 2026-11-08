//! Tests for `lanstash_adapter_admin_api::guard`: what a browser sends from another site, or
//! to a name that is not the network's own, is refused.

use lanstash_adapter_admin_api::guard::{https_redirect, refused};
use lanstash_domain::http::RequestHead;

fn req(method: &str, headers: &[(&str, &str)]) -> RequestHead {
    let headers = headers.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect();
    RequestHead { method: method.into(), target: "/api/v1/restart?force=1".into(), minor_version: 1, headers }
}

#[test]
fn the_plain_port_sends_a_browser_to_https_only_on_the_networks_own_names() {
    let at = |host: &str, target: &str| {
        let r = RequestHead {
            method: "GET".into(),
            target: target.into(),
            minor_version: 1,
            headers: vec![("host".into(), host.into())],
        };
        https_redirect(&r, 8443)
    };
    assert_eq!(at("10.0.0.10:8081", "/platforms/steam").as_deref(), Some("https://10.0.0.10:8443/platforms/steam"));
    assert_eq!(at("cache.lan", "/").as_deref(), Some("https://cache.lan:8443/"));
    assert_eq!(at("[fd00::5]:8081", "/x").as_deref(), Some("https://[fd00::5]:8443/x"));
    assert_eq!(at("evil.example.com", "/"), None, "never to a name someone outside could own");
    assert_eq!(at("cache.lan", "//evil.example.com/"), None, "never a path that reads as another host");
    assert_eq!(at("cache.lan", "*"), None);
    assert_eq!(at("cache.lan@evil", "/"), None);
}

#[test]
fn the_ui_on_its_own_origin_and_tools_without_one_pass() {
    let host = ("host", "10.0.0.10:8081");
    assert_eq!(refused(&req("POST", &[host, ("origin", "http://10.0.0.10:8081")])), None);
    assert_eq!(refused(&req("POST", &[host, ("sec-fetch-site", "same-origin")])), None);
    assert_eq!(refused(&req("POST", &[host])), None, "the command line sends no Origin");
    assert_eq!(refused(&req("POST", &[])), None);
    // A dev server proxying the API keeps the browser's Host and Origin.
    assert_eq!(refused(&req("PUT", &[("host", "localhost:5173"), ("origin", "http://localhost:5173")])), None);
}

#[test]
fn a_state_change_from_another_site_is_refused() {
    let host = ("host", "10.0.0.10:8081");
    for origin in ["http://evil.example", "null", "http://10.0.0.10:9999"] {
        assert!(refused(&req("POST", &[host, ("origin", origin)])).is_some(), "{origin}");
    }
    assert!(refused(&req("DELETE", &[host, ("sec-fetch-site", "cross-site")])).is_some());
    assert!(refused(&req("POST", &[host, ("sec-fetch-site", "same-site")])).is_some());
    assert_eq!(refused(&req("GET", &[host, ("origin", "http://evil.example")])), None, "reading changes nothing");
}

#[test]
fn only_the_networks_own_names_are_answered() {
    for host in ["10.0.0.10:8081", "[fd00::2]:8081", "localhost:8081", "lanstash:8081", "box.lan", "nas.home.arpa"] {
        assert_eq!(refused(&req("GET", &[("host", host)])), None, "{host}");
    }
    for host in ["rebind.evil.example:8081", "lanstash.example.com", "[not-v6]:80", ":8081"] {
        assert!(refused(&req("GET", &[("host", host)])).is_some(), "{host}");
    }
}
