//! Tests for `lanstash_adapter_crypto::certificate`: a certificate signed by its own new key.

use lanstash_adapter_crypto::certificate::make;
use lanstash_domain::x509::AltName;
use ring::signature::{ECDSA_P256_SHA256_ASN1, UnparsedPublicKey};

/// A DER element's header length and content length, from its first bytes.
fn element(der: &[u8]) -> (usize, usize) {
    match der[1] {
        n @ 0..=0x7F => (2, usize::from(n)),
        0x81 => (3, usize::from(der[2])),
        _ => (4, usize::from(der[2]) << 8 | usize::from(der[3])),
    }
}

#[test]
fn the_certificate_is_signed_by_its_own_key() {
    let made = make("lanstash", &[AltName::Dns("lanstash.lan".into())], 1_794_141_296_000).unwrap();
    // Certificate ::= SEQUENCE { tbs, algorithm, signature BIT STRING }.
    let (outer, _) = element(&made.certificate);
    let rest = &made.certificate[outer..];
    let (h, n) = element(rest);
    let tbs = &rest[..h + n];
    let after = &rest[h + n..];
    let (ah, an) = element(after);
    let sig_der = &after[ah + an..];
    let (sh, sn) = element(sig_der);
    let signature = &sig_der[sh + 1..sh + sn];
    // The key is in the body: the 65 bytes after the BIT STRING header 03 42 00.
    let at = tbs.windows(3).position(|w| w == [0x03, 0x42, 0x00]).unwrap() + 3;
    let public = UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, &tbs[at..at + 65]);
    assert!(public.verify(tbs, signature).is_ok());
    assert!(!made.key.is_empty());
}

#[test]
fn every_certificate_has_a_key_of_its_own() {
    let a = make("lanstash", &[], 0).unwrap();
    let b = make("lanstash", &[], 0).unwrap();
    assert_ne!(a.key, b.key);
    assert_ne!(a.certificate, b.certificate);
}
