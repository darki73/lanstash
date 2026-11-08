//! Tests for `lanstash_domain::x509`: DER lengths, times, and the certificate's shape.

use lanstash_domain::x509::{AltName, CertSpec, certificate, der, time, to_be_signed};

#[test]
fn lengths_are_short_or_long_as_der_says() {
    assert_eq!(der(0x04, &[1, 2]), [0x04, 2, 1, 2]);
    assert_eq!(&der(0x04, &[0; 200])[..3], [0x04, 0x81, 200]);
    assert_eq!(&der(0x04, &[0; 300])[..4], [0x04, 0x82, 0x01, 0x2C]);
}

#[test]
fn times_are_utc_until_2050_and_generalized_after() {
    // 2026-11-08 12:34:56 UTC.
    assert_eq!(time(1_794_141_296_000), [&[0x17, 13][..], b"261108123456Z"].concat());
    // 2051-01-01 00:00:00 UTC.
    assert_eq!(time(2_556_143_999_000 + 1_000), [&[0x18, 15][..], b"20510101000000Z"].concat());
    assert_eq!(time(0), [&[0x17, 13][..], b"700101000000Z"].concat());
}

#[test]
fn a_certificate_holds_its_key_names_and_signature() {
    let key = [4u8; 65];
    let names = [AltName::Dns("lanstash.lan".into()), AltName::V4([192, 0, 2, 7])];
    let spec = CertSpec {
        serial: [0xFF; 16],
        not_before_ms: 0,
        not_after_ms: 1_000,
        common_name: "lanstash",
        names: &names,
        public_key: &key,
    };
    let tbs = to_be_signed(&spec);
    assert_eq!(tbs[0], 0x30);
    let has = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
    assert!(has(&tbs, &[0xA0, 3, 0x02, 1, 2]), "version 3");
    assert!(has(&tbs, &[0x02, 16, 0x7F]), "a positive serial without a leading zero");
    assert!(has(&tbs, &[&[0x03, 66, 0][..], &key[..]].concat()), "the public key");
    assert!(has(&tbs, &[&[0x82, 12][..], &b"lanstash.lan"[..]].concat()), "the host name");
    assert!(has(&tbs, &[0x87, 4, 192, 0, 2, 7]), "the address");
    let cert = certificate(&tbs, &[9, 9, 9]);
    assert_eq!(cert[0], 0x30);
    assert!(cert.ends_with(&[0x03, 4, 0, 9, 9, 9]), "the signature last");
}
