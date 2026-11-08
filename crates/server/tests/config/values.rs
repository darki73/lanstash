//! Tests for `lanstash::config::values`: the kinds of value a key takes.

use lanstash::config::values::{list, size};

#[test]
fn sizes_take_a_suffix_in_powers_of_1024() {
    assert_eq!(
        [size("4096"), size("512K"), size("1G"), size("2t")],
        [Some(4096), Some(512 << 10), Some(1 << 30), Some(2 << 40)]
    );
    assert_eq!(size("lots"), None);
    assert_eq!(size("99999999999T"), None, "too large is no size");
}

#[test]
fn lists_are_split_on_commas_without_empty_entries() {
    assert_eq!(list(" a, b ,,c ").collect::<Vec<_>>(), ["a", "b", "c"]);
}
