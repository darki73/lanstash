//! Tests for `lanstash_app::once_per`: a result is worked out once per key.

use std::cell::Cell;

use lanstash_app::once_per::OncePer;

#[test]
fn the_same_key_takes_the_kept_result_another_works_it_out_again() {
    let worked = Cell::new(0);
    let once: OncePer<(u64, u64), String> = OncePer::default();
    let work = |s: &str| {
        worked.set(worked.get() + 1);
        s.to_string()
    };
    assert_eq!(*once.get((1, 1), || work("a")), "a");
    assert_eq!(*once.get((1, 1), || work("b")), "a", "nothing changed: the kept result");
    assert_eq!(worked.get(), 1);
    assert_eq!(*once.get((2, 1), || work("c")), "c", "the stash changed");
    assert_eq!(*once.get((1, 1), || work("d")), "d", "only the last key is kept");
    assert_eq!(worked.get(), 3);
}
