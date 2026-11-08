//! Tests for `lanstash_adapter_crypto::digest`: SHA-256 against its published test vector.

use lanstash_adapter_crypto::digest::Sha256;
use lanstash_domain::accounts::hex;
use lanstash_ports::digest::Digest;

#[test]
fn sha256_of_abc() {
    assert_eq!(hex(&Sha256.sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}
