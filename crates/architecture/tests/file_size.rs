mod support;

use lanstash_architecture::file_size::check_file_sizes;
use support::{file, krate};

#[test]
fn files_over_the_ceiling_are_reported_unless_exempt() {
    let mut c = krate("crates/x", "x", &[]);
    c.sources = vec![file("short.rs", "a\nb\n"), file("long.rs", "a\nb\nc\nd\n")];
    c.tests = vec![file("long.rs", "a\nb\nc\n")];
    let v: Vec<String> = check_file_sizes(&c, 2, &["crates/x/src/long.rs"]).into_iter().map(|v| v.detail).collect();
    assert_eq!(v, vec!["crates/x/tests/long.rs has 3 lines (limit 2)"]);
    assert_eq!(check_file_sizes(&c, 2, &[]).len(), 2);
}
