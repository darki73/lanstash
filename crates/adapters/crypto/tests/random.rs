//! Tests for `lanstash_adapter_crypto::random`: the system's generator fills what it is given.

use lanstash_adapter_crypto::random::SystemRng;
use lanstash_ports::random::Random;

#[test]
fn bytes_are_drawn_and_differ() {
    let (mut a, mut b) = ([0u8; 32], [0u8; 32]);
    SystemRng.fill(&mut a).unwrap();
    SystemRng.fill(&mut b).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, [0u8; 32]);
}
