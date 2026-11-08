//! Counter groups list every counter by field name, in declaration order.

use std::sync::atomic::Ordering;

use lanstash_app::passthrough::PassCounters;
use lanstash_app::serve::ServeCounters;

#[test]
fn values_follow_the_declaration() {
    let c = PassCounters::default();
    c.bytes.fetch_add(5, Ordering::Relaxed);
    assert_eq!(
        c.values(),
        vec![("requests", 0), ("bytes", 5), ("errors", 0), ("timeouts", 0), ("too_large", 0), ("aborted", 0)]
    );
    let names: Vec<&str> = ServeCounters::default().values().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names[..3], ["requests", "hits", "misses"]);
    assert!(names.contains(&"partial"), "documented fields are counters too");
}
