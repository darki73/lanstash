//! Tests for `lanstash_ports::digest`: the trait works as an object.

use lanstash_ports::digest::Digest;

/// Not SHA-256: the length in every byte, enough to show the contract.
struct Length;

impl Digest for Length {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        [u8::try_from(data.len()).unwrap_or(u8::MAX); 32]
    }
}

#[test]
fn a_digest_is_32_bytes() {
    let d: &dyn Digest = &Length;
    assert_eq!(d.sha256(b"abc"), [3; 32]);
}
