//! Tests for `lanstash_domain::segment::meta`.

use lanstash_domain::segment::meta::*;

#[test]
fn round_trips_and_skips_unsafe_pairs() {
    let m = encode_meta(&[
        ("Content-Type", "application/x-steam-chunk"),
        ("x-content-crc", "123"),
        ("bad=name", "v"),
        ("x-split", "a\r\nInjected: 1"),
    ]);
    assert_eq!(m, b"content-type=application/x-steam-chunk\nx-content-crc=123\n");
    let pairs: Vec<_> = decode_meta(&m).collect();
    assert_eq!(pairs, vec![("content-type", "application/x-steam-chunk"), ("x-content-crc", "123")]);
    assert_eq!(decode_meta(b"\xff\xfe").count(), 0);
    assert_eq!(decode_meta(b"noequals\n=x\nk=v=w").collect::<Vec<_>>(), vec![("k", "v=w")]);
}
