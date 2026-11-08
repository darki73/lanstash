//! Tests for `lanstash_adapter_admin_api::api`'s answers: an error is JSON on an
//! API path and a line of text elsewhere, and a method not taken says which are.

use super::*;

#[test]
fn errors_are_json_on_the_api_and_text_elsewhere() {
    let a = api();
    let on_api = get(&a, "GET", "/api/v1/nothing");
    assert_eq!(on_api.status, 404);
    assert!(on_api.headers.contains(&("Content-Type".into(), "application/json".into())));
    assert!(on_api.body.contains("\"kind\":\"not-found\""), "{}", on_api.body);
    let elsewhere = get(&a, "GET", "/nothing");
    assert_eq!((elsewhere.status, elsewhere.body.as_str()), (404, "not found\n"));
    assert!(elsewhere.headers.contains(&("Content-Type".into(), "text/plain".into())));
    let method = get(&a, "POST", "/api/v1/version");
    assert_eq!(method.status, 405);
    assert!(method.headers.contains(&("Allow".into(), "GET, HEAD".into())));
}
