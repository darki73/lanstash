//! Tests for `lanstash_adapter_admin_api::api`'s health path: the checks the
//! service makes of itself, made when asked.

use std::sync::atomic::{AtomicUsize, Ordering};

use lanstash_app::health::{Check, Level};

use super::*;

#[test]
fn the_checks_are_made_when_asked() {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&asked);
    let a = api().with_health(Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
        vec![Check::new(Level::Warn, "open files", "lanstash: 900 of 1024 (88 %)")]
    }));
    let got = get(&a, "GET", "/api/v1/health");
    assert_eq!(got.status, 200);
    assert_eq!(
        got.body,
        "{\"checks\":[{\"level\":\"warn\",\"what\":\"open files\",\"detail\":\"lanstash: 900 of 1024 (88 %)\"}]}\n"
    );
    get(&a, "GET", "/api/v1/health");
    assert_eq!(asked.load(Ordering::Relaxed), 2, "not kept: made each time");
}

#[test]
fn without_checks_the_path_is_not_found() {
    assert_eq!(get(&api(), "GET", "/api/v1/health").status, 404);
}
