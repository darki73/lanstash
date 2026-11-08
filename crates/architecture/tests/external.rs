//! Tests for `lanstash_architecture::external`: the allow-list is sorted, has
//! no duplicates, and is the one the policy uses.

use lanstash_architecture::external::ALLOWED;
use lanstash_architecture::policy::LANSTASH;

#[test]
fn the_allow_list_is_sorted_once_and_is_the_policys() {
    let mut sorted = ALLOWED.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, ALLOWED, "sorted, each crate once");
    assert_eq!(LANSTASH.external, ALLOWED);
}
