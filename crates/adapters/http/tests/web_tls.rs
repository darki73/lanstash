//! Tests for `lanstash_adapter_http::web_tls`: fingerprints, and the client's
//! setup.

use lanstash_adapter_http::web_tls::{client, fingerprint, tidy};

#[test]
fn a_fingerprint_is_the_sha256_in_hex_pairs_and_a_typed_one_is_tidied() {
    // The SHA-256 of "abc" (FIPS 180-2's example).
    assert_eq!(
        fingerprint(b"abc"),
        "BA:78:16:BF:8F:01:CF:EA:41:41:40:DE:5D:AE:22:23:B0:03:61:A3:96:17:7A:9C:B4:10:FF:61:F2:00:15:AD"
    );
    assert_eq!(tidy("ba7816bf 8f01"), "BA:78:16:BF:8F:01");
    assert_eq!(tidy("BA:78:16"), "BA:78:16");
}

#[test]
fn a_client_is_made_for_a_name_or_an_address_and_not_for_nonsense() {
    assert!(client("dns.home", None, false).is_ok());
    assert!(client("10.0.0.53", Some("AB:CD"), false).is_ok());
    assert!(client("10.0.0.53", None, true).is_ok());
    assert!(client("not a host!", None, false).is_err());
}
