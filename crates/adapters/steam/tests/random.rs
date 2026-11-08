//! Tests for `lanstash_adapter_steam::random`.

use lanstash_adapter_steam::random::RingRandom;
use lanstash_ports::random::Random;

#[test]
fn two_fills_differ() {
    let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
    RingRandom.fill(&mut a).unwrap();
    RingRandom.fill(&mut b).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, [0u8; 32]);
}
